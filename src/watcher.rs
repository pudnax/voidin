use color_eyre::eyre::Result;
use notify_debouncer_mini::DebouncedEventKind;
use winit::event_loop::EventLoopProxy;

use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    time::Duration,
};

use crate::{utils::ImportResolver, SHADER_FOLDER};

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

                let mut resolver = ImportResolver::new(&[SHADER_FOLDER]);
                let source = match resolver.populate(&path) {
                    Ok(s) => s.contents,
                    Err(err) => {
                        log::error!("Failed to parse imports: {err}");
                        return;
                    }
                };
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
