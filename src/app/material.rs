use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use glam::{vec4, Vec4};

use crate::{
    utils::{NonZeroSized, ResizableBuffer, ResizableBufferExt},
    Gpu,
};

use super::{
    bind_group_layout::{self, WrappedBindGroupLayout},
    texture::TextureId,
};

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct MaterialId(u32);

impl MaterialId {
    pub fn new(id: u32) -> Self {
        Self(id)
    }
}

#[repr(C)]
#[derive(Copy, Clone, Default, Pod, Zeroable)]
pub struct Material {
    pub base_color: Vec4,
    pub albedo: TextureId,
    pub normal: TextureId,
    pub metallic_roughness: TextureId,
    pub emissive: TextureId,
}

pub struct MaterialPool {
    pub(crate) buffer: ResizableBuffer<Material>,

    pub(crate) bind_group_layout: bind_group_layout::BindGroupLayout,
    pub(crate) bind_group: wgpu::BindGroup,

    gpu: Arc<Gpu>,
}

impl MaterialPool {
    pub fn new(gpu: Arc<Gpu>) -> Self {
        let buffer = gpu.device().create_resizable_buffer_init(
            wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            &[Material {
                base_color: vec4(1., 1., 1., 1.),
                ..Default::default()
            }],
        );

        let bind_group_layout =
            gpu.device()
                .create_bind_group_layout_wrap(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("MaterialPool: Bind Group Layout"),
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

        let bind_group = Self::create_bind_group(gpu.device(), &bind_group_layout, &buffer);

        Self {
            buffer,
            bind_group_layout,
            bind_group,

            gpu,
        }
    }

    pub fn create_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        materials: &ResizableBuffer<Material>,
    ) -> wgpu::BindGroup {
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("MaterialPool: Bind Group"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: materials.as_entire_binding(),
            }],
        });

        bind_group
    }

    pub fn add(&mut self, material: Material) -> MaterialId {
        let was_resized = self.buffer.push(&self.gpu, &[material]);

        if was_resized {
            self.bind_group =
                Self::create_bind_group(self.gpu.device(), &self.bind_group_layout, &self.buffer);
        }

        log::info!("Added material with id: {}", self.buffer.len() as u32 - 1);
        MaterialId(self.buffer.len() as u32 - 1)
    }
}
