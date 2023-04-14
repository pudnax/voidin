use std::sync::Arc;

use glam::Vec4;

use crate::{
    utils::{NonZeroSized, ResizableBuffer, ResizableBufferExt},
    Gpu,
};

use super::{
    bind_group_layout::{self, WrappedBindGroupLayout},
    texture::TextureId,
};

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaterialId(u32);

#[repr(C)]
#[derive(Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Material {
    pub base_color: Vec4,
    pub albedo: TextureId,
    pub normal: TextureId,
    pub metallic_roughness: TextureId,
    pub emissive: TextureId,
}

pub struct MaterialManager {
    buffer: ResizableBuffer<Material>,

    pub(crate) bind_group_layout: bind_group_layout::BindGroupLayout,
    pub(crate) bind_group: wgpu::BindGroup,

    gpu: Arc<Gpu>,
}

impl MaterialManager {
    pub fn new(gpu: Arc<Gpu>) -> Self {
        let buffer = gpu.device().create_resizable_buffer_init(
            wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            &[Material::default()],
        );

        let bind_group_layout =
            gpu.device()
                .create_bind_group_layout_wrap(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("MaterialManager: Bind Group Layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT
                            | wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: Some(Material::NSIZE),
                        },
                        count: None,
                    }],
                });

        let bind_group = gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("MaterialManager: Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        Self {
            buffer,
            bind_group_layout,
            bind_group,

            gpu,
        }
    }

    pub fn add(&mut self, material: Material) -> MaterialId {
        let was_resized = self
            .buffer
            .push(self.gpu.device(), self.gpu.queue(), &[material]);

        if was_resized {
            let desc = &wgpu::BindGroupDescriptor {
                label: Some("MaterialManager: Bind Group"),
                layout: &self.bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.buffer.as_entire_binding(),
                }],
            };
            self.bind_group = self.gpu.device().create_bind_group(desc);
        }

        MaterialId(self.buffer.len() as u32 - 1)
    }
}