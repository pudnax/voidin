use std::time::Duration;

use bvh::{Bvh, BvhBuilder, BvhNode};
use color_eyre::Result;
use voidin::*;

#[allow(dead_code)]
struct Demo {
    pipeline: RenderHandle,

    vertices: ResizableBuffer<Vec4>,
    indices: ResizableBuffer<UVec4>,

    bvh: Bvh,
    bvh_nodes: ResizableBuffer<BvhNode>,

    geometry_bind_group: wgpu::BindGroup,
}

impl Example for Demo {
    fn name() -> &'static str {
        "Bvh GPU"
    }

    fn init(app: &mut App) -> Result<Self> {
        let geometry_bgl =
            app.device()
                .create_bind_group_layout_wrap(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Trace BGL"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: Some(Vec4::NSIZE),
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: Some(UVec4::NSIZE),
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: Some(BvhNode::NSIZE),
                            },
                            count: None,
                        },
                    ],
                });
        let camera_binding = app.world.get::<CameraUniformBinding>()?;
        let pipeline = app
            .get_pipeline_arena_mut()
            .process_render_pipeline_from_path(
                "src/bin/bvh_trace.wgsl",
                pipeline::RenderPipelineDescriptor {
                    layout: vec![
                        camera_binding.bind_group_layout.clone(),
                        geometry_bgl.clone(),
                    ],
                    depth_stencil: None,
                    ..Default::default()
                },
            )?;

        let mut vertices = vec![];
        let mut indices = vec![];
        let (bnuuy, _) = tobj::load_obj("assets/bunny.obj", &tobj::GPU_LOAD_OPTIONS)?;
        for mesh in bnuuy.into_iter().map(|m| m.mesh) {
            vertices.extend(
                mesh.positions
                    .chunks_exact(3)
                    .map(|v| Vec3::from_slice(v).extend(0.)),
            );
            indices.extend(
                mesh.indices
                    .chunks_exact(3)
                    .map(|i| UVec3::from_slice(i).extend(0)),
            );
        }

        let bvh = BvhBuilder::new(&vertices, &mut indices).build();

        let vertices = app.device().create_resizable_buffer_init(
            bytemuck::cast_slice(&vertices),
            wgpu::BufferUsages::STORAGE,
        );
        let indices = app.device().create_resizable_buffer_init(
            bytemuck::cast_slice(&indices),
            wgpu::BufferUsages::STORAGE,
        );
        let bvh_nodes = app
            .device()
            .create_resizable_buffer_init(&bvh.nodes, wgpu::BufferUsages::STORAGE);

        let geometry_bind_group = app.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Geometry Bind Group"),
            layout: &geometry_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: vertices.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: indices.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: bvh_nodes.as_entire_binding(),
                },
            ],
        });

        Ok(Self {
            pipeline,
            vertices,
            indices,
            bvh,
            bvh_nodes,
            geometry_bind_group,
        })
    }

    fn update(&mut self, _ctx: UpdateContext) {}

    fn resize(&mut self, _gpu: &Gpu, _width: u32, _height: u32) {}

    fn render(&mut self, mut ctx: RenderContext) {
        let camera = ctx.world.unwrap::<CameraUniformBinding>();
        let arena = ctx.world.unwrap::<PipelineArena>();
        let mut pass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: ctx.view_target.main_view(),
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.13,
                        g: 0.13,
                        b: 0.13,
                        a: 1.0,
                    }),
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
        });

        pass.set_pipeline(arena.get_pipeline(self.pipeline));
        pass.set_bind_group(0, &camera.binding, &[]);
        pass.set_bind_group(1, &self.geometry_bind_group, &[]);
        pass.draw(0..3, 0..1);
        drop(pass);

        ctx.ui(|egui_ctx| {
            egui::Window::new("debug").show(egui_ctx, |ui| {
                ui.label(format!(
                    "Fps: {:.04?}",
                    Duration::from_secs_f64(ctx.app_state.dt)
                ));
            });
        });
    }
}

fn main() -> Result<()> {
    let window = WindowBuilder::new();

    let camera = Camera::new(vec3(-0.16, 0.75, 1.5), 0., 0.);
    run::<Demo>(window, camera)
}
