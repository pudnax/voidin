use std::time::Instant;

use color_eyre::Result;
use dolly::prelude::YawPitch;
use log::{info, warn};
use poisson_corrode::{
    app::{App, AppState},
    camera::Camera,
};
use wgpu::SurfaceError;
use winit::{
    dpi::{LogicalSize, PhysicalSize},
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::ControlFlow,
};

const UPDATES_PER_SECOND: u32 = 60;
const FIXED_TIME_STEP: f64 = 1. / UPDATES_PER_SECOND as f64;
const MAX_FRAME_TIME: f64 = 15. * FIXED_TIME_STEP; // 0.25;

fn main() -> Result<()> {
    color_eyre::install()?;

    let event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::WindowBuilder::new()
        .with_title("Poisson Corrode")
        .with_inner_size(LogicalSize::new(1280, 1024))
        .with_resizable(false)
        .with_decorations(false)
        .build(&event_loop)?;

    let PhysicalSize { width, height } = window.inner_size();
    let camera = Camera::new(width, height);

    let mut app = App::new(&window)?;
    let mut app_state = AppState::new(camera);
    let info = app.get_info();
    println!("{info}");

    let mut current_instant = Instant::now();
    let mut accumulated_time = 0.;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

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
                    app_state.update(FIXED_TIME_STEP);
                    app.update(&mut app_state);

                    accumulated_time -= FIXED_TIME_STEP;
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
            Event::WindowEvent { event, window_id } if window_id == window.id() => match event {
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            virtual_keycode: Some(key),
                            state: ElementState::Pressed,
                            ..
                        },
                    ..
                } => {
                    if key == VirtualKeyCode::Z {
                        app_state
                            .camera
                            .rig
                            .driver_mut::<YawPitch>()
                            .rotate_yaw_pitch(-90., 0.0);
                    }
                    if key == VirtualKeyCode::X {
                        app_state
                            .camera
                            .rig
                            .driver_mut::<YawPitch>()
                            .rotate_yaw_pitch(90., 0.0);
                    }
                }
                _ => {}
            },
            Event::RedrawRequested(_) => {
                if let Err(err) = app.render(&app_state) {
                    eprintln!("get_current_texture error: {:?}", err);
                    match err {
                        SurfaceError::Lost | SurfaceError::Outdated => {
                            warn!("render: Outdated Surface");
                            app.surface.configure(&app.device, &app.surface_config);
                            window.request_redraw();
                        }
                        SurfaceError::OutOfMemory => *control_flow = ControlFlow::Exit,
                        SurfaceError::Timeout => info!("Surface Timeout"),
                    }
                }
            }
            Event::RedrawEventsCleared => window.request_redraw(),
            _ => {}
        }
    })
}
