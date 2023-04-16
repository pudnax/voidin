use std::time::Instant;

use color_eyre::Result;
use glam::vec3;
use log::warn;
use voidin::{
    app::{state::AppState, App},
    camera::Camera,
    input::{KeyMap, KeyboardMap},
    watcher::Watcher,
};
use wgpu::SurfaceError;
use winit::{
    dpi::{LogicalSize, PhysicalSize},
    event::{Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::ControlFlow,
};

const UPDATES_PER_SECOND: u32 = 60;
const FIXED_TIME_STEP: f64 = 1. / UPDATES_PER_SECOND as f64;
const MAX_FRAME_TIME: f64 = 15. * FIXED_TIME_STEP; // 0.25;

fn main() -> Result<()> {
    color_eyre::install()?;
    env_logger::builder()
        .filter_module("wgpu_core", log::LevelFilter::Warn)
        .filter_module("wgpu_hal", log::LevelFilter::Warn)
        .filter_module("mangohud", log::LevelFilter::Warn)
        .filter_module("winit", log::LevelFilter::Warn)
        .filter_module("naga", log::LevelFilter::Error)
        .init();

    let event_loop = winit::event_loop::EventLoopBuilder::with_user_event().build();
    let window = winit::window::WindowBuilder::new()
        .with_title("Voidin")
        .with_inner_size(LogicalSize::new(1280, 1024))
        // .with_resizable(false)
        // .with_decorations(false)
        .build(&event_loop)?;

    let PhysicalSize { width, height } = window.inner_size();

    let camera = Camera::new(vec3(2., 6., 3.), 45., 0., width, height);
    use VirtualKeyCode::*;
    let keyboard_map = KeyboardMap::new()
        .bind(W, KeyMap::new("move_fwd", 1.0))
        .bind(S, KeyMap::new("move_fwd", -1.0))
        .bind(D, KeyMap::new("move_right", 1.0))
        .bind(A, KeyMap::new("move_right", -1.0))
        .bind(Q, KeyMap::new("move_up", 1.0))
        .bind(E, KeyMap::new("move_up", -1.0))
        .bind(LShift, KeyMap::new("boost", 1.0))
        .bind(LControl, KeyMap::new("boost", -1.0));
    let mut app_state = AppState::new(camera, Some(keyboard_map));

    let watcher = Watcher::new(event_loop.create_proxy())?;

    let mut app = App::new(&window, watcher)?;
    let info = app.get_info();
    println!("{info}");

    app.setup_scene()?;

    let mut current_instant = Instant::now();
    let mut accumulated_time = 0.;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        app_state.input.update(&event, &window);
        match event {
            Event::MainEventsCleared => {
                let new_instant = Instant::now();
                let frame_time = new_instant
                    .duration_since(current_instant)
                    .as_secs_f64()
                    .min(MAX_FRAME_TIME);
                current_instant = new_instant;

                accumulated_time += frame_time;
                while accumulated_time >= FIXED_TIME_STEP {
                    let actions = app_state.update(FIXED_TIME_STEP);
                    app.update(&app_state, actions);

                    accumulated_time -= FIXED_TIME_STEP;
                }
            }
            Event::RedrawEventsCleared => window.request_redraw(),
            Event::RedrawRequested(_) => {
                if let Err(err) = app.render(&app_state) {
                    eprintln!("get_current_texture error: {:?}", err);
                    match err {
                        SurfaceError::Lost | SurfaceError::Outdated => {
                            warn!("render: Outdated Surface");
                            app.surface.configure(app.device(), &app.surface_config);
                            window.request_redraw();
                        }
                        SurfaceError::OutOfMemory => *control_flow = ControlFlow::Exit,
                        SurfaceError::Timeout => warn!("Surface Timeout"),
                    }
                }
            }
            Event::WindowEvent {
                event:
                    WindowEvent::Resized(PhysicalSize { width, height })
                    | WindowEvent::ScaleFactorChanged {
                        new_inner_size: &mut PhysicalSize { width, height },
                        ..
                    },
                ..
            } => {
                if width != 0 && height != 0 {
                    app_state.camera.aspect = width as f32 / height as f32;
                    app.resize(width, height);
                }
            }
            Event::WindowEvent {
                event:
                    WindowEvent::CloseRequested
                    | WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                virtual_keycode: Some(VirtualKeyCode::Escape),
                                ..
                            },
                        ..
                    },
                ..
            } => *control_flow = ControlFlow::Exit,
            Event::UserEvent((path, source)) => {
                app.handle_events(path, source);
            }
            Event::LoopDestroyed => {
                println!("// End from the loop. Bye bye~⏎ ");
            }
            _ => {}
        }
    })
}
