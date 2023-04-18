use color_eyre::Result;
use std::path::Path;
use wgpu::CommandEncoder;

use super::Pass;

use crate::{
    app::{
        bind_group_layout::WrappedBindGroupLayout,
        global_ubo::GlobalUniformBinding,
        pipeline::{PipelineArena, RenderHandle, RenderPipelineDescriptor},
        ViewTarget,
    },
    utils::world::World,
};

pub struct PostProcess {
    pipeline: RenderHandle,
}

impl PostProcess {
    pub fn new(world: &mut World, path: impl AsRef<Path>) -> Result<Self> {
        let global_ubo = world.get::<GlobalUniformBinding>()?;
        let mut pipeline_arena = world.get_mut::<PipelineArena>()?;
        let postprocess_bind_group_layout =
            world
                .device()
                .create_bind_group_layout_wrap(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Post Process Bind Group Layout"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT
                                | wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT
                                | wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                    ],
                });
        let desc = RenderPipelineDescriptor {
            label: Some("Post Process Pipeline".into()),
            layout: vec![global_ubo.layout.clone(), postprocess_bind_group_layout],
            depth_stencil: None,
            ..Default::default()
        };
        let pipeline = pipeline_arena.process_render_pipeline_from_path(path, desc)?;
        Ok(Self { pipeline })
    }
}

pub struct PostProcessResource<'a> {
    pub sampler: &'a wgpu::Sampler,
}

impl Pass for PostProcess {
    type Resoutces<'a> = PostProcessResource<'a>;

    fn record(
        &self,
        world: &World,
        encoder: &mut CommandEncoder,
        view_target: &ViewTarget,
        resource: Self::Resoutces<'_>,
    ) {
        let global_ubo = world.unwrap::<GlobalUniformBinding>();
        let post_process_target = view_target.post_process_write();
        let arena = world.unwrap::<PipelineArena>();
        let tex_bind_group = arena
            .device()
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Post: Texture Bind Group"),
                layout: &arena.get_descriptor(self.pipeline).layout[1],
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(post_process_target.source),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(resource.sampler),
                    },
                ],
            });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Post Process Pass"),
            color_attachments: &[Some(post_process_target.get_color_attachment(
                wgpu::Color {
                    r: 0.,
                    g: 0.,
                    b: 0.,
                    a: 0.0,
                },
            ))],
            depth_stencil_attachment: None,
        });
        pass.set_bind_group(0, &global_ubo.binding, &[]);
        pass.set_bind_group(1, &tex_bind_group, &[]);
        pass.set_pipeline(arena.get_pipeline(self.pipeline));
        pass.draw(0..3, 0..1);
    }
}
