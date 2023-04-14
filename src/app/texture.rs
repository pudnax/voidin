use crate::utils;

use super::{
    bind_group_layout::{self, WrappedBindGroupLayout},
    DEFAULT_SAMPLER_DESC,
};

#[repr(C)]
#[derive(Debug, Copy, Default, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TextureId(u32);

pub struct TextureManager {
    pub views: Vec<wgpu::TextureView>,

    sampler: wgpu::Sampler,
    pub bind_group_layout: bind_group_layout::BindGroupLayout,
}

const MAX_TEXTURES: u32 = 1 << 10;

impl TextureManager {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let views = vec![
            utils::create_solid_color_texture(device, queue, glam::Vec4::splat(1.))
                .create_view(&Default::default()),
        ];

        let bind_group_layout =
            device.create_bind_group_layout_wrap(&wgpu::BindGroupLayoutDescriptor {
                label: Some("TextureManager: bind group layout"),
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
        let sampler = device.create_sampler(&DEFAULT_SAMPLER_DESC);

        Self {
            views,

            sampler,
            bind_group_layout,
        }
    }

    pub fn add(&mut self, view: wgpu::TextureView) -> TextureId {
        self.views.push(view);

        TextureId(self.views.len() as u32 - 1)
    }

    pub fn create_bind_group(&self, device: &wgpu::Device) -> wgpu::BindGroup {
        let views = (0..MAX_TEXTURES as _)
            .map(|i| self.views.get(i).unwrap_or(&self.views[0]))
            .collect::<Vec<_>>();

        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("TextureManager: bind group"),
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
