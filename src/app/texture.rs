use std::sync::Arc;

use crate::{utils, Gpu};

use super::{
    bind_group_layout::{self, WrappedBindGroupLayout},
    DEFAULT_SAMPLER_DESC,
};

#[repr(C)]
#[derive(Debug, Copy, Default, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TextureId(u32);

impl TextureId {
    pub fn id(&self) -> u32 {
        self.0
    }
}

pub struct TexturePool {
    pub views: Vec<wgpu::TextureView>,

    sampler: wgpu::Sampler,
    pub bind_group_layout: bind_group_layout::BindGroupLayout,
    pub bind_group: wgpu::BindGroup,

    gpu: Arc<Gpu>,
}

const MAX_TEXTURES: u32 = 1 << 10;

impl TexturePool {
    pub fn new(gpu: Arc<Gpu>) -> Self {
        let views = vec![utils::create_solid_color_texture(
            gpu.device(),
            gpu.queue(),
            glam::Vec4::splat(0.),
        )
        .create_view(&Default::default())];

        let bind_group_layout =
            gpu.device()
                .create_bind_group_layout_wrap(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("TexturePool: bind group layout"),
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
                            count: core::num::NonZeroU32::new(MAX_TEXTURES),
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT
                                | wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: core::num::NonZeroU32::new(MAX_TEXTURES),
                        },
                    ],
                });
        let sampler = gpu.device().create_sampler(&DEFAULT_SAMPLER_DESC);

        let bind_views = (0..MAX_TEXTURES as _)
            .map(|i| views.get(i).unwrap_or(&views[0]))
            .collect::<Vec<_>>();

        let bind_group = gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("TexturePool: bind group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureViewArray(&bind_views),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        Self {
            views,

            sampler,
            bind_group_layout,
            bind_group,
            gpu,
        }
    }

    pub fn add(&mut self, view: wgpu::TextureView) -> TextureId {
        self.views.push(view);

        TextureId(self.views.len() as u32 - 1)
    }

    pub fn update_bind_group(&mut self) {
        let views: Vec<_> = self.views.iter().collect();

        self.bind_group = self
            .gpu
            .device()
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("TexturePool: bind group"),
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureViewArray(&views),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            })
    }
}
