pub mod app;
pub mod camera;
pub mod input;
pub mod models;
pub mod pass;
pub mod shader_compiler;
pub mod utils;
pub mod watcher;

use shader_compiler::ShaderCompiler;

use once_cell::sync::Lazy;
use parking_lot::Mutex;

pub const SCREENSHOTS_FOLDER: &str = "screenshots/";

// Global shader compiler with application specific flags not compatible with wgpu
pub(crate) static SHADER_COMPILER: Lazy<Mutex<ShaderCompiler>> =
    Lazy::new(|| Mutex::new(ShaderCompiler::new()));

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
