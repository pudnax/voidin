use color_eyre::eyre::Result;
use notify_debouncer_mini::DebouncedEventKind;
use winit::event_loop::EventLoop;

use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    time::Duration,
};

use crate::{shader_compiler::CompilerError, SHADER_COMPILER};

pub type SpirvBytes = Vec<u32>;

pub trait ReloadablePipeline {
    fn reload(&mut self, device: &wgpu::Device, module: &wgpu::ShaderModule);
}

pub struct Watcher {
    watcher: notify_debouncer_mini::Debouncer<notify::RecommendedWatcher>,
}

impl Watcher {
    pub fn new(event_loop: &EventLoop<(PathBuf, SpirvBytes)>) -> Result<Self> {
        let watcher = notify_debouncer_mini::new_debouncer(
            Duration::from_millis(100),
            None,
            watch_callback(event_loop),
        )?;

        Ok(Self { watcher })
    }

    pub fn watch_file(&mut self, path: impl AsRef<Path>) -> Result<()> {
        self.watcher
            .watcher()
            .watch(path.as_ref(), notify::RecursiveMode::NonRecursive)?;
        Ok(())
    }
}

fn watch_callback(
    event_loop: &EventLoop<(PathBuf, SpirvBytes)>,
) -> impl FnMut(notify_debouncer_mini::DebounceEventResult) {
    let proxy = event_loop.create_proxy();
    move |event| match event {
        Ok(events) => {
            if let Some(path) = events
                .into_iter()
                .filter(|e| e.kind == DebouncedEventKind::Any)
                .map(|event| event.path)
                .next()
            {
                assert_eq!(
                    path.extension(),
                    Some(OsStr::new("wgsl")),
                    "TODO: Support glsl shaders."
                );
                match SHADER_COMPILER.lock().create_shader_module(&path) {
                    Ok(module) => {
                        log::info!(
                            "Shader successfully compiled: {}",
                            path.file_name()
                                .and_then(|name| name.to_str())
                                .unwrap_or("")
                        );
                        proxy
                            .send_event((path, module))
                            .expect("Event Loop has been dropped");
                    }
                    Err(err) => match err {
                        CompilerError::Compile { error, source } => {
                            let file_name =
                                path.file_name().and_then(|x| x.to_str()).unwrap_or("wgsl");
                            error.emit_to_stderr_with_path(&source, file_name);
                        }
                        _ => eprintln!("{err}"),
                    },
                }
            }
        }
        Err(errs) => errs.into_iter().for_each(|err| {
            eprintln!("File watcher error: {err}");
        }),
    }
}
