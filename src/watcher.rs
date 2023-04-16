use color_eyre::eyre::Result;
use notify_debouncer_mini::DebouncedEventKind;
use winit::event_loop::EventLoopProxy;

use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    time::Duration,
};

use crate::SHADER_FOLDER;

pub struct Watcher {
    watcher: notify_debouncer_mini::Debouncer<notify::RecommendedWatcher>,
}

impl Watcher {
    pub fn new(proxy: EventLoopProxy<(PathBuf, String)>) -> Result<Self> {
        let watcher = notify_debouncer_mini::new_debouncer(
            Duration::from_millis(100),
            None,
            watch_callback(proxy),
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
    proxy: EventLoopProxy<(PathBuf, String)>,
) -> impl FnMut(notify_debouncer_mini::DebounceEventResult) {
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

                let parser = ocl_include::Parser::builder()
                    .add_source(
                        ocl_include::source::Fs::builder()
                            .include_dir(uni_path::Path::new(SHADER_FOLDER))
                            .expect("Shader folder doesn't exist")
                            .build(),
                    )
                    .build();

                let Ok(parsed_res) = parser
                    .parse(uni_path::Path::new(&path.to_string_lossy())) else {
                        log::error!("Failed to read file: {}", path.display());
                        return;
                    } ;
                let source = parsed_res.collect().0;
                proxy
                    .send_event((path, source))
                    .expect("Event Loop has been dropped");
            }
        }
        Err(errs) => errs.into_iter().for_each(|err| {
            eprintln!("File watcher error: {err}");
        }),
    }
}
