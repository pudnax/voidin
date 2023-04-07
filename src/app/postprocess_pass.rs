use color_eyre::Result;
use std::path::Path;
use wgpu::CommandEncoder;

use crate::{
    bind_group_layout::{BindGroupLayout, WrappedBindGroupLayout},
    pipeline::{Arena, RenderHandle, RenderPipelineDescriptor},
    view_target::ViewTarget,
    Pass,
};

pub struct PostProcessPipeline {
    pipeline: RenderHandle,
}

impl PostProcessPipeline {
    pub fn new(
        pipeline_arena: &mut Arena,
        global_uniform: BindGroupLayout,
        path: impl AsRef<Path>,
    ) -> Result<Self> {
        let postprocess_bind_group_layout =
            pipeline_arena
                .device
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
            layout: vec![global_uniform, postprocess_bind_group_layout],
            depth_stencil: None,
            ..Default::default()
        };
        let pipeline = pipeline_arena.process_render_pipeline_from_path(path, desc)?;
        Ok(Self { pipeline })
    }
}

pub struct PostProcessResource<'a> {
    pub arena: &'a Arena,
    pub global_binding: &'a wgpu::BindGroup,
    pub sampler: &'a wgpu::Sampler,
}

impl Pass for PostProcessPipeline {
    type Resoutces<'a> = PostProcessResource<'a>;

    fn record<'a>(
        &self,
        encoder: &mut CommandEncoder,
        view_target: &ViewTarget,
        resource: Self::Resoutces<'a>,
    ) {
        let post_process_target = view_target.post_process_write();
        let tex_bind_group = resource
            .arena
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Post: Texture Bind Group"),
                layout: &resource.arena.get_descriptor(self.pipeline).layout[1],
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&post_process_target.source),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&resource.sampler),
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
        pass.set_bind_group(0, &resource.global_binding, &[]);
        pass.set_bind_group(1, &tex_bind_group, &[]);
        pass.set_pipeline(resource.arena.get_pipeline(self.pipeline));
        pass.draw(0..3, 0..1);
    }
}
