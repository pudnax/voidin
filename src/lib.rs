#![allow(clippy::new_without_default)]

use app::{
    bind_group_layout::{self, WrappedBindGroupLayout},
    global_ubo::{GlobalUniformBinding, Uniform},
};
use camera::{CameraUniform, CameraUniformBinding};
use utils::NonZeroSized;
pub mod app;
pub mod camera;
pub mod input;
pub mod models;
pub mod pass;
pub mod recorder;
pub mod utils;
pub mod watcher;

pub const UPDATES_PER_SECOND: u32 = 60;
pub const FIXED_TIME_STEP: f64 = 1. / UPDATES_PER_SECOND as f64;
pub const MAX_FRAME_TIME: f64 = 15. * FIXED_TIME_STEP; // 0.25;

pub const SCREENSHOTS_FOLDER: &str = "screenshots";
pub const VIDEO_FOLDER: &str = "recordings";
pub const SHADER_FOLDER: &str = "shaders";

#[derive(Debug)]
pub struct Gpu {
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl Gpu {
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    pub fn adapter(&self) -> &wgpu::Adapter {
        &self.adapter
    }
}

pub struct GlobalsBindGroup {
    layout: bind_group_layout::BindGroupLayout,
    binding: wgpu::BindGroup,
}

impl GlobalsBindGroup {
    pub fn new(gpu: &Gpu, globals: &GlobalUniformBinding, camera: &CameraUniformBinding) -> Self {
        let layout = gpu.device().create_bind_group_layout_wrap(&Self::LAYOUT);
        let binding = gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Globals Bind Group"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: globals.buffer().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: camera.buffer().as_entire_binding(),
                },
            ],
        });
        Self { layout, binding }
    }

    const LAYOUT: wgpu::BindGroupLayoutDescriptor<'_> = wgpu::BindGroupLayoutDescriptor {
        label: Some("Globals Bind Group Layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT.union(wgpu::ShaderStages::COMPUTE),
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: Some(Uniform::NSIZE),
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT.union(wgpu::ShaderStages::COMPUTE),
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: Some(CameraUniform::NSIZE),
                },
                count: None,
            },
        ],
    };

    pub fn binding(&self) -> &wgpu::BindGroup {
        &self.binding
    }
}
