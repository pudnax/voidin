use std::sync::Arc;

use crate::{
    app::bind_group_layout::WrappedBindGroupLayout,
    utils::{NonZeroSized, ResizableBuffer},
    Gpu,
};

use bytemuck::{Pod, Zeroable};

use super::bind_group_layout;

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, Pod, Zeroable)]
pub struct Light {
    pub position: glam::Vec3,
    pub radius: f32,
    pub color: glam::Vec3,
    _padding: u32,
}

impl Light {
    pub fn new(position: glam::Vec3, radius: f32, color: glam::Vec3) -> Self {
        Self {
            position,
            radius,
            color,
            _padding: 0,
        }
    }
}

pub struct LightPool {
    pub(crate) buffer: ResizableBuffer<Light>,

    pub(crate) bind_group_layout: bind_group_layout::BindGroupLayout,
    pub(crate) bind_group: wgpu::BindGroup,

    gpu: Arc<Gpu>,
}

impl LightPool {
    pub fn new(gpu: Arc<Gpu>) -> Self {
        let buffer = ResizableBuffer::new(
            gpu.device(),
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::VERTEX,
        );

        let bind_group_layout =
            gpu.device()
                .create_bind_group_layout_wrap(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Light Pool Bind Group Layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: Some(Light::NSIZE),
                        },
                        count: None,
                    }],
                });
        let bind_group = Self::create_bind_group(&gpu, &bind_group_layout, &buffer);

        Self {
            buffer,

            bind_group_layout,
            bind_group,
            gpu: gpu.clone(),
        }
    }

    fn create_bind_group(
        gpu: &Gpu,
        bind_group_layout: &wgpu::BindGroupLayout,
        lights: &ResizableBuffer<Light>,
    ) -> wgpu::BindGroup {
        gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Light Pool Bind Group"),
            layout: bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: lights.as_tight_binding(),
            }],
        })
    }

    pub fn add(&mut self, lights: &[Light]) {
        self.buffer.push(&self.gpu, lights);
        self.bind_group = Self::create_bind_group(&self.gpu, &self.bind_group_layout, &self.buffer);
    }
}
