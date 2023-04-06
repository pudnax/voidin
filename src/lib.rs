pub mod app;
pub mod bind_group_layout;
pub mod camera;
pub mod gltf;
pub mod input;
pub mod pipeline;
pub mod shader_compiler;
pub mod utils;
pub mod view_target;
pub mod watcher;

use shader_compiler::ShaderCompiler;

use once_cell::sync::Lazy;
use parking_lot::Mutex;
pub static SHADER_COMPILER: Lazy<Mutex<ShaderCompiler>> =
    Lazy::new(|| Mutex::new(ShaderCompiler::default()));
