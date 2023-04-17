use std::path::Path;

use color_eyre::Result;
use glam::{Vec2, Vec3};
use wgpu::{util::align_to, IndexFormat};

use super::Pass;

use crate::{
    app::{
        bind_group_layout::BindGroupLayout,
        global_ubo::GlobalUniformBinding,
        instance::InstancesManager,
        material::MaterialManager,
        mesh::MeshManager,
        pipeline::{
            self, Arena, ComputeHandle, ComputePipelineDescriptor, RenderHandle,
            RenderPipelineDescriptor,
        },
        texture::TextureManager,
    },
    camera::CameraUniformBinding,
    utils::{DrawIndexedIndirect, Ref, ResizableBuffer, World},
};

pub struct Geometry {
    pipeline: RenderHandle,

    textures: Ref<TextureManager>,
    meshes: Ref<MeshManager>,
    materials: Ref<MaterialManager>,
    instances: Ref<InstancesManager>,
    global_ubo: Ref<GlobalUniformBinding>,
    camera: Ref<CameraUniformBinding>,
}

impl Geometry {
    pub fn new(world: &World, pipeline_arena: &mut Arena) -> Result<Self> {
        let path = Path::new("shaders").join("draw_indirect.wgsl");
        let textures = world.get::<TextureManager>();
        let meshes = world.get::<MeshManager>();
        let materials = world.get::<MaterialManager>();
        let instances = world.get::<InstancesManager>();
        let global_ubo = world.get::<GlobalUniformBinding>();
        let camera = world.get::<CameraUniformBinding>();
        let render_desc = RenderPipelineDescriptor {
            label: Some("Geometry Pipeline".into()),
            layout: vec![
                global_ubo.get().layout.clone(),
                camera.get().bind_group_layout.clone(),
                textures.get().bind_group_layout.clone(),
                meshes.get().mesh_info_layout.clone(),
                instances.get().bind_group_layout.clone(),
                materials.get().bind_group_layout.clone(),
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
        Ok(Self {
            pipeline,
            textures,
            meshes,
            materials,
            instances,
            global_ubo,
            camera,
        })
    }
}

pub struct GeometryResource<'a> {
    pub arena: &'a Arena,
    pub depth_texture: &'a wgpu::TextureView,

    pub draw_cmd_buffer: &'a ResizableBuffer<DrawIndexedIndirect>,
}

impl Pass for Geometry {
    type Resoutces<'a> = GeometryResource<'a>;
    fn record(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view_target: &crate::app::ViewTarget,
        resources: Self::Resoutces<'_>,
    ) {
        let meshes = self.meshes.get();
        let textures = self.textures.get();
        let materials = self.materials.get();
        let instances = self.instances.get();
        let global_ubo = self.global_ubo.get();
        let camera = self.camera.get();

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
        rpass.set_bind_group(0, &global_ubo.binding, &[]);
        rpass.set_bind_group(1, &camera.binding, &[]);
        rpass.set_bind_group(2, &textures.bind_group, &[]);
        rpass.set_bind_group(3, &meshes.mesh_info_bind_group, &[]);
        rpass.set_bind_group(4, &instances.bind_group, &[]);
        rpass.set_bind_group(5, &materials.bind_group, &[]);

        rpass.set_vertex_buffer(0, meshes.vertices.full_slice());
        rpass.set_vertex_buffer(1, meshes.normals.full_slice());
        rpass.set_vertex_buffer(2, meshes.tex_coords.full_slice());
        rpass.set_index_buffer(meshes.indices.full_slice(), IndexFormat::Uint32);
        rpass.multi_draw_indexed_indirect(
            &resources.draw_cmd_buffer,
            0,
            resources.draw_cmd_buffer.len() as _,
        );
    }
}

pub struct EmitDraws {
    pipeline: ComputeHandle,
    meshes: Ref<MeshManager>,
    instances: Ref<InstancesManager>,
}

impl EmitDraws {
    pub fn new(
        world: &World,
        pipeline_arena: &mut Arena,
        draw_cmd_layout: BindGroupLayout,
    ) -> Result<Self> {
        let meshes = world.get::<MeshManager>();
        let instances = world.get::<InstancesManager>();
        let path = Path::new("shaders").join("emit_draws.wgsl");
        let comp_desc = ComputePipelineDescriptor {
            label: Some("Compute Indirect Pipeline".into()),
            layout: vec![
                meshes.get().mesh_info_layout.clone(),
                instances.get().bind_group_layout.clone(),
                draw_cmd_layout.clone(),
            ],
            push_constant_ranges: vec![],
            entry_point: "emit_draws".into(),
        };
        let pipeline = pipeline_arena.process_compute_pipeline_from_path(path, comp_desc)?;
        Ok(Self {
            pipeline,
            meshes,
            instances,
        })
    }
}

pub struct EmitDrawsResource<'a> {
    pub arena: &'a Arena,
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
        let meshes = self.meshes.get();
        let instances = self.instances.get();
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Emit Draws Pass"),
        });

        cpass.set_pipeline(resources.arena.get_pipeline(self.pipeline));
        cpass.set_bind_group(0, &meshes.mesh_info_bind_group, &[]);
        cpass.set_bind_group(1, &instances.bind_group, &[]);
        cpass.set_bind_group(2, resources.draw_cmd_bind_group, &[]);
        let num_dispatches = align_to(resources.draw_cmd_buffer.len() as _, 64) / 64;
        cpass.dispatch_workgroups(num_dispatches, 1, 1);
    }
}
