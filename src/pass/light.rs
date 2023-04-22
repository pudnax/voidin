use std::path::Path;

use bytemuck::{Pod, Zeroable};
use color_eyre::Result;
use glam::Vec3;
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    PrimitiveState,
};

use crate::{
    app::{
        gbuffer::GBuffer,
        pipeline::{self, PipelineArena, RenderHandle, RenderPipelineDescriptor},
        ViewTarget,
    },
    camera::CameraUniformBinding,
    models::make_uv_sphere,
    utils::{world::World, NonZeroSized, ResizableBuffer},
};

use super::Pass;

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, Pod, Zeroable)]
pub struct Light {
    pub position: glam::Vec3,
    pub radius: f32,
    pub color: glam::Vec3,
}

pub struct LightPass {
    stencil_pipeline: RenderHandle,
    lighting_pipeline: RenderHandle,

    vertex_count: u32,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
}

impl LightPass {
    pub fn ner(world: &World, gbuffer: &GBuffer) -> Result<Self> {
        let device = world.gpu.device();
        let camera = world.get::<CameraUniformBinding>()?;
        let sphere = make_uv_sphere(1.0, 1);

        let vertices = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Light: sphere vertices buffer"),
            contents: bytemuck::cast_slice(&sphere.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let indices = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Light sphere indices buffer"),
            contents: bytemuck::cast_slice(&sphere.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let shader_path = Path::new("shaders").join("lights.wgsl");
        let vertex_buffers_layouts = vec![
            pipeline::VertexBufferLayout {
                array_stride: Light::SIZE as _,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: wgpu::vertex_attr_array![
                    0 => Float32x3, // Position
                    1 => Float32,   // Radius
                    2 => Float32x3, // Color
                ]
                .to_vec(),
            },
            pipeline::VertexBufferLayout {
                array_stride: Vec3::SIZE as _,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: wgpu::vertex_attr_array![3 => Float32x3].to_vec(),
            },
        ];

        let stensil_desc = RenderPipelineDescriptor {
            label: Some("Light: stensil pipeline".into()),
            layout: vec![camera.bind_group_layout.clone()],
            vertex: pipeline::VertexState {
                entry_point: "vs_main_stensil".into(),
                buffers: vertex_buffers_layouts.clone(),
            },
            fragment: None,
            primitive: PrimitiveState {
                unclipped_depth: true,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24PlusStencil8,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState {
                    front: wgpu::StencilFaceState {
                        compare: wgpu::CompareFunction::Always,
                        fail_op: wgpu::StencilOperation::Keep,
                        depth_fail_op: wgpu::StencilOperation::DecrementWrap,
                        pass_op: wgpu::StencilOperation::Keep,
                    },
                    back: wgpu::StencilFaceState {
                        compare: wgpu::CompareFunction::Always,
                        fail_op: wgpu::StencilOperation::Keep,
                        depth_fail_op: wgpu::StencilOperation::IncrementWrap,
                        pass_op: wgpu::StencilOperation::Keep,
                    },
                    read_mask: 0,
                    write_mask: 0xFF,
                },
                bias: wgpu::DepthBiasState::default(),
            }),
            ..Default::default()
        };
        let stencil_pipeline = world
            .get_mut::<PipelineArena>()?
            .process_render_pipeline_from_path(&shader_path, stensil_desc)?;

        let lighting_desc = RenderPipelineDescriptor {
            label: Some("Light: render pipeline".into()),
            layout: vec![
                camera.bind_group_layout.clone(),
                gbuffer.bind_group_layout.clone(),
            ],
            vertex: pipeline::VertexState {
                entry_point: "vs_main_lighting".into(),
                buffers: vertex_buffers_layouts,
            },
            fragment: Some(pipeline::FragmentState {
                entry_point: "fs_main_lighting".into(),
                targets: vec![Some(wgpu::ColorTargetState {
                    format: ViewTarget::FORMAT,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: Default::default(),
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Front),
                unclipped_depth: true,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24PlusStencil8,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilState {
                    front: wgpu::StencilFaceState {
                        compare: wgpu::CompareFunction::NotEqual,
                        fail_op: wgpu::StencilOperation::Keep,
                        depth_fail_op: wgpu::StencilOperation::Keep,
                        pass_op: wgpu::StencilOperation::Keep,
                    },
                    back: wgpu::StencilFaceState {
                        compare: wgpu::CompareFunction::NotEqual,
                        fail_op: wgpu::StencilOperation::Keep,
                        depth_fail_op: wgpu::StencilOperation::Keep,
                        pass_op: wgpu::StencilOperation::Keep,
                    },
                    read_mask: 0xFF,
                    write_mask: 0,
                },
                bias: wgpu::DepthBiasState::default(),
            }),
            ..Default::default()
        };
        let lighting_pipeline = world
            .get_mut::<PipelineArena>()?
            .process_render_pipeline_from_path(shader_path, lighting_desc)?;

        Ok(Self {
            stencil_pipeline,
            lighting_pipeline,

            vertex_count: sphere.indices.len() as _,
            vertices,
            indices,
        })
    }
}

pub struct LightingResource<'a> {
    pub gbuffer: &'a GBuffer,
}

impl Pass for LightPass {
    type Resoutces<'a> = LightingResource<'a>;

    fn record(
        &self,
        world: &World,
        encoder: &mut wgpu::CommandEncoder,
        view_target: &crate::app::ViewTarget,
        resources: Self::Resoutces<'_>,
    ) {
        let arena = world.unwrap::<PipelineArena>();
        let camera = world.unwrap::<CameraUniformBinding>();
        let lights = world.unwrap::<ResizableBuffer<Light>>();

        let mut stencil_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Ligts Stensil Pass"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &resources.gbuffer.depth,
                depth_ops: None,
                stencil_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0),
                    store: true,
                }),
            }),
        });

        stencil_pass.set_pipeline(arena.get_pipeline(self.stencil_pipeline));
        stencil_pass.set_bind_group(0, &camera.binding, &[]);

        stencil_pass.set_vertex_buffer(0, lights.full_slice());
        stencil_pass.set_vertex_buffer(1, self.vertices.slice(..));
        stencil_pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint16);

        stencil_pass.draw_indexed(0..self.vertex_count, 0, 0..lights.len() as _);
        drop(stencil_pass);

        let mut lighting_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Lights Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view_target.main_view(),
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &resources.gbuffer.depth,
                depth_ops: None,
                stencil_ops: None,
            }),
        });

        lighting_pass.set_pipeline(arena.get_pipeline(self.lighting_pipeline));
        lighting_pass.set_bind_group(0, &camera.binding, &[]);
        lighting_pass.set_bind_group(1, &resources.gbuffer.bind_group, &[]);

        lighting_pass.set_vertex_buffer(0, lights.full_slice());
        lighting_pass.set_vertex_buffer(1, self.vertices.slice(..));
        lighting_pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint16);

        lighting_pass.draw_indexed(0..self.vertex_count, 0, 0..lights.len() as _);

        drop(lighting_pass);
    }
}
