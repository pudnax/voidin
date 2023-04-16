use std::path::Path;

use color_eyre::Result;
use glam::{Vec2, Vec3};
use wgpu::IndexFormat;

use super::Pass;

use crate::{
    app::{
        bind_group_layout::BindGroupLayout,
        mesh::MeshManager,
        pipeline::{
            self, Arena, ComputeHandle, ComputePipelineDescriptor, RenderHandle,
            RenderPipelineDescriptor,
        },
    },
    utils::{align_to, DrawIndexedIndirect, ResizableBuffer},
};

pub struct Geometry {
    pipeline: RenderHandle,
}

impl Geometry {
    pub fn new(
        pipeline_arena: &mut Arena,
        global_uniform_layout: BindGroupLayout,
        camera_uniform_layout: BindGroupLayout,
        texture_layout: BindGroupLayout,
        mesh_layout: BindGroupLayout,
        instance_layout: BindGroupLayout,
        material_layout: BindGroupLayout,
    ) -> Result<Self> {
        let path = Path::new("shaders").join("draw_indirect.wgsl");
        let render_desc = RenderPipelineDescriptor {
            label: Some("Geometry Pipeline".into()),
            layout: vec![
                global_uniform_layout.clone(),
                camera_uniform_layout.clone(),
                texture_layout.clone(),
                mesh_layout.clone(),
                instance_layout.clone(),
                material_layout.clone(),
            ],
            vertex: pipeline::VertexState {
                entry_point: "vs_main".into(),
                buffers: vec![
                    // Positions
                    pipeline::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Vec3>() as _,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: wgpu::vertex_attr_array![0 => Float32x3].to_vec(),
                    },
                    // Normals
                    pipeline::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Vec3>() as _,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: wgpu::vertex_attr_array![1 => Float32x3].to_vec(),
                    },
                    // UVs
                    pipeline::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Vec2>() as _,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: wgpu::vertex_attr_array![2 => Float32x2].to_vec(),
                    },
                ],
            },
            fragment: Some(pipeline::FragmentState {
                entry_point: "fs_main".into(),
                ..Default::default()
            }),
            ..Default::default()
        };
        let pipeline = pipeline_arena.process_render_pipeline_from_path(path, render_desc)?;
        Ok(Self { pipeline })
    }
}

pub struct GeometryResource<'a> {
    pub arena: &'a Arena,
    pub depth_texture: &'a wgpu::TextureView,
    pub global_binding: &'a wgpu::BindGroup,
    pub camera_binding: &'a wgpu::BindGroup,
    pub textures_bind_group: &'a wgpu::BindGroup,
    pub instance_bind_group: &'a wgpu::BindGroup,
    pub material_bind_group: &'a wgpu::BindGroup,

    pub draw_cmd_buffer: &'a ResizableBuffer<DrawIndexedIndirect>,
    pub mesh_manager: &'a MeshManager,
}

impl Pass for Geometry {
    type Resoutces<'a> = GeometryResource<'a>;
    fn record(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view_target: &crate::app::ViewTarget,
        resources: Self::Resoutces<'_>,
    ) {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Geometry Pass"),
            color_attachments: &[Some(view_target.get_color_attachment(wgpu::Color {
                r: 0.13,
                g: 0.13,
                b: 0.13,
                a: 0.0,
            }))],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &resources.depth_texture,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0.0),
                    store: true,
                }),
                stencil_ops: None,
            }),
        });

        rpass.set_pipeline(resources.arena.get_pipeline(self.pipeline));
        rpass.set_bind_group(0, &resources.global_binding, &[]);
        rpass.set_bind_group(1, &resources.camera_binding, &[]);
        rpass.set_bind_group(2, &resources.textures_bind_group, &[]);
        rpass.set_bind_group(3, &resources.mesh_manager.mesh_info_bind_group, &[]);
        rpass.set_bind_group(4, &resources.instance_bind_group, &[]);
        rpass.set_bind_group(5, &resources.material_bind_group, &[]);

        rpass.set_vertex_buffer(0, resources.mesh_manager.vertices.full_slice());
        rpass.set_vertex_buffer(1, resources.mesh_manager.normals.full_slice());
        rpass.set_vertex_buffer(2, resources.mesh_manager.tex_coords.full_slice());
        rpass.set_index_buffer(
            resources.mesh_manager.indices.full_slice(),
            IndexFormat::Uint32,
        );
        rpass.multi_draw_indexed_indirect(
            &resources.draw_cmd_buffer,
            0,
            resources.draw_cmd_buffer.len() as _,
        );
    }
}

pub struct EmitDraws {
    pipeline: ComputeHandle,
}

impl EmitDraws {
    pub fn new(
        pipeline_arena: &mut Arena,
        mesh_layout: BindGroupLayout,
        instance_layout: BindGroupLayout,
        draw_cmd_layout: BindGroupLayout,
    ) -> Result<Self> {
        let path = Path::new("shaders").join("emit_draws.wgsl");
        let comp_desc = ComputePipelineDescriptor {
            label: Some("Compute Indirect Pipeline".into()),
            layout: vec![
                mesh_layout.clone(),
                instance_layout.clone(),
                draw_cmd_layout.clone(),
            ],
            push_constant_ranges: vec![],
            entry_point: "emit_draws".into(),
        };
        let pipeline = pipeline_arena.process_compute_pipeline_from_path(path, comp_desc)?;
        Ok(Self { pipeline })
    }
}

pub struct EmitDrawsResource<'a> {
    pub arena: &'a Arena,
    pub mesh_info_bind_group: &'a wgpu::BindGroup,
    pub instance_bind_group: &'a wgpu::BindGroup,
    pub draw_cmd_bind_group: &'a wgpu::BindGroup,
    pub draw_cmd_buffer: &'a ResizableBuffer<DrawIndexedIndirect>,
}

impl Pass for EmitDraws {
    type Resoutces<'a> = EmitDrawsResource<'a>;

    fn record(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        _view_target: &crate::app::ViewTarget,
        resources: Self::Resoutces<'_>,
    ) {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Emit Draws Pass"),
        });

        cpass.set_pipeline(resources.arena.get_pipeline(self.pipeline));
        cpass.set_bind_group(0, resources.mesh_info_bind_group, &[]);
        cpass.set_bind_group(1, resources.instance_bind_group, &[]);
        cpass.set_bind_group(2, resources.draw_cmd_bind_group, &[]);
        cpass.dispatch_workgroups(align_to(resources.draw_cmd_buffer.len() as _, 32), 1, 1);
    }
}
