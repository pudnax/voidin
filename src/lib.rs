pub mod app;
pub mod camera;
pub mod gltf;
pub mod input;
pub mod shader_compiler;
pub mod utils;
pub mod watcher;

use shader_compiler::ShaderCompiler;

use once_cell::sync::Lazy;
use parking_lot::Mutex;

pub static SHADER_COMPILER: Lazy<Mutex<ShaderCompiler>> =
    Lazy::new(|| Mutex::new(ShaderCompiler::default()));

pub trait Pass {
    type Resoutces<'a>;

    fn record(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view_target: &app::ViewTarget,
        resources: Self::Resoutces<'_>,
    );
}
