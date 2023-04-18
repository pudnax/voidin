#![allow(clippy::new_without_default)]
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
