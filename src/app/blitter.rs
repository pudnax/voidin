use std::{cell::RefCell, collections::HashMap, num::NonZeroU32};

use super::DEFAULT_SAMPLER_DESC;

pub struct Blitter {
    pipelines: RefCell<HashMap<wgpu::TextureFormat, wgpu::RenderPipeline>>,
    shader: wgpu::ShaderModule,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

impl Blitter {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Blit Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/shaders/blit.wgsl"
            )))),
        });
        let sampler = device.create_sampler(&DEFAULT_SAMPLER_DESC);
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Blit Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipelines = RefCell::new(HashMap::from([(
            wgpu::TextureFormat::Bgra8UnormSrgb,
            Self::create_pipeline(device, &shader, wgpu::TextureFormat::Bgra8UnormSrgb),
        )]));

        Self {
            pipelines,
            shader,
            bind_group_layout,
            sampler,
        }
    }

    pub fn _blit_to_texture(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        src_texture: &wgpu::Texture,
        dst_texture: &wgpu::TextureView,
    ) {
        let mut pipelines = self.pipelines.borrow_mut();
        let pipeline = pipelines
            .entry(src_texture.format())
            .or_insert_with_key(|&format| Self::create_pipeline(device, &self.shader, format));

        let src_texture_view = src_texture.create_view(&Default::default());
        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&src_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Blit Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: dst_texture,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
        });

        render_pass.set_pipeline(pipeline);
        render_pass.set_bind_group(0, &texture_bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }

    pub fn generate_mipmaps(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        texture: &wgpu::Texture,
    ) {
        let mut pipelines = self.pipelines.borrow_mut();
        let pipeline = pipelines
            .entry(texture.format())
            .or_insert_with_key(|&format| Self::create_pipeline(device, &self.shader, format));

        let mip_count = texture.mip_level_count();
        let array_count = texture.depth_or_array_layers();

        for array_layer in 0..array_count {
            for (mip_from, mip_to) in (0..mip_count).zip(1..mip_count) {
                let src_view = texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("Blit: Src View"),
                    base_mip_level: mip_from,
                    mip_level_count: NonZeroU32::new(1),
                    base_array_layer: array_layer,
                    array_layer_count: NonZeroU32::new(1),
                    ..Default::default()
                });
                let dst_view = texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("Blit: Dst View"),
                    base_mip_level: mip_to,
                    mip_level_count: NonZeroU32::new(1),
                    base_array_layer: array_layer,
                    array_layer_count: NonZeroU32::new(1),
                    ..Default::default()
                });

                let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Blit: Bind Group"),
                    layout: &self.bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&src_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&self.sampler),
                        },
                    ],
                });

                let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: None,
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &dst_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                            store: true,
                        },
                    })],
                    depth_stencil_attachment: None,
                });
                rpass.set_pipeline(pipeline);
                rpass.set_bind_group(0, &bind_group, &[]);
                rpass.draw(0..3, 0..1);
            }
        }
    }

    fn create_pipeline(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Blit Pipeline"),
            layout: None,
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: "vs_main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: "fs_main",
                targets: &[Some(format.into())],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        })
    }
}
