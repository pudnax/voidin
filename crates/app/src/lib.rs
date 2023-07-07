#![allow(clippy::new_without_default)]

use color_eyre::Result;
use components::FpsCounter;
use std::time::Instant;

use glam::vec3;
use log::warn;
use wgpu::SurfaceError;
use winit::{
    dpi::PhysicalSize,
    event::{Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::ControlFlow,
};

pub use crate::app::App;
mod app;
pub mod models;
pub mod pass;
pub mod prelude;

pub use crate::models::GltfDocument;
pub use app::DEFAULT_SAMPLER_DESC;
pub use app::{
    gbuffer::GBuffer,
    global_ubo::{GlobalUniformBinding, GlobalsBindGroup, Uniform},
    pipeline,
    state::AppState,
    ProfilerCommandEncoder, RenderContext, UpdateContext, ViewTarget,
};
pub use components::{
    bind_group_layout::{self, WrappedBindGroupLayout},
    Camera, Gpu, LerpExt, NonZeroSized, ResizableBuffer, ResizableBufferExt, Watcher,
    {CameraUniform, CameraUniformBinding}, {KeyMap, KeyboardMap},
};
pub use egui;
pub use pools::*;
pub use winit::{dpi::LogicalSize, window::WindowBuilder};

pub const UPDATES_PER_SECOND: u32 = 60;
pub const FIXED_TIME_STEP: f64 = 1. / UPDATES_PER_SECOND as f64;
pub const MAX_FRAME_TIME: f64 = 15. * FIXED_TIME_STEP; // 0.25;

pub const SHADER_FOLDER: &str = "shaders";

pub trait Example: 'static + Sized {
    fn name() -> &'static str {
        "Example"
    }

    fn init(gpu: &mut App) -> Result<Self>;
    fn setup_scene(&mut self, _app: &mut App) -> Result<()> {
        Ok(())
    }
    fn update(&mut self, _ctx: UpdateContext) {}
    fn resize(&mut self, _gpu: &Gpu, _width: u32, _height: u32) {}
    fn render(&mut self, ctx: RenderContext);
}

pub fn run_default<E: Example>() -> color_eyre::Result<()> {
    let window = winit::window::WindowBuilder::new()
        .with_title(E::name())
        .with_inner_size(LogicalSize::new(1280, 1024));

    let camera = Camera::new(vec3(0., 0., 0.), 0., 0.);
    run::<E>(window, camera)
}

pub fn run<E: Example>(
    window_builder: WindowBuilder,
    mut camera: Camera,
) -> color_eyre::Result<()> {
    color_eyre::install()?;
    env_logger::builder()
        .parse_env(env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"))
        .filter_module("wgpu_core", log::LevelFilter::Warn)
        .filter_module("wgpu_hal", log::LevelFilter::Warn)
        .filter_module("MANGOHUD", log::LevelFilter::Warn)
        .filter_module("winit", log::LevelFilter::Warn)
        .filter_module("naga", log::LevelFilter::Error)
        .init();

    let event_loop = winit::event_loop::EventLoopBuilder::with_user_event().build();
    let window = window_builder.build(&event_loop)?;

    let PhysicalSize { width, height } = window.inner_size();
    camera.aspect = width as f32 / height as f32;

    let keyboard_map = {
        use VirtualKeyCode::*;
        KeyboardMap::new()
            .bind(W, KeyMap::new("move_fwd", 1.0))
            .bind(S, KeyMap::new("move_fwd", -1.0))
            .bind(D, KeyMap::new("move_right", 1.0))
            .bind(A, KeyMap::new("move_right", -1.0))
            .bind(Q, KeyMap::new("move_up", 1.0))
            .bind(E, KeyMap::new("move_up", -1.0))
            .bind(LShift, KeyMap::new("boost", 1.0))
            .bind(LControl, KeyMap::new("boost", -1.0))
    };
    let mut app_state = AppState::new(camera, Some(keyboard_map));

    let watcher = Watcher::new(event_loop.create_proxy())?;

    let mut app = App::new(&window, watcher)?;
    let info = app.get_info();
    println!("{info}");

    let mut example = E::init(&mut app)?;

    let now = std::time::Instant::now();
    example.setup_scene(&mut app)?;
    app.setup_scene()?;
    println!("Scene finished: {:?}", now.elapsed());

    let mut current_instant = Instant::now();
    let mut accumulated_time = 0.;
    let mut fps_counter = FpsCounter::new();

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

                let mut actions = vec![];
                accumulated_time += frame_time;
                while accumulated_time >= FIXED_TIME_STEP {
                    app_state.input.tick();
                    actions.extend(app_state.update(FIXED_TIME_STEP));

                    accumulated_time -= FIXED_TIME_STEP;
                }
                app.update(&mut app_state, actions, |ctx| example.update(ctx))
                    .unwrap();
                app_state.input.mouse_state.refresh();
            }
            Event::RedrawEventsCleared => window.request_redraw(),
            Event::RedrawRequested(_) => {
                app_state.dt = fps_counter.record();
                if let Err(err) = app.render(&window, &app_state, |ctx| example.render(ctx)) {
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
                    example.resize(&app.gpu, width, height);
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
            Event::DeviceEvent { event, .. } => app_state.input.on_device_event(&event),
            Event::WindowEvent { event, .. } => {
                if app.egui_state.on_event(&app.egui_context, &event).consumed {
                    return;
                }

                app_state.input.on_window_event(&window, &event);
            }
            Event::UserEvent(path) => {
                app.handle_events(path);
            }
            Event::LoopDestroyed => {
                println!("// End from the loop. Bye bye~⏎ ");
            }
            _ => {}
        }
    })
}
