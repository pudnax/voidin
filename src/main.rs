use color_eyre::Result;
use wgpu::SurfaceError;
use winit::{
    dpi::PhysicalSize,
    event::{Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::ControlFlow,
};

mod state;

fn main() -> Result<()> {
    color_eyre::install()?;

    let event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::WindowBuilder::new()
        .with_title("Poisson Corrode")
        .with_inner_size(PhysicalSize::new(1280, 1024))
        .with_resizable(false)
        .with_decorations(false)
        .build(&event_loop)?;

    let mut state = state::State::new(&window)?;
    let info = state.get_info();
    println!("{info}");

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::MainEventsCleared => {
                window.request_redraw();
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
                    state.resize(width, height);
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
            Event::RedrawRequested(_) => {
                if let Err(err) = state.render() {
                    eprintln!("get_current_texture error: {:?}", err);
                    match err {
                        SurfaceError::Lost | SurfaceError::Outdated => {
                            state
                                .surface
                                .configure(&state.device, &state.surface_config);
                            window.request_redraw();
                        }
                        SurfaceError::OutOfMemory => {
                            *control_flow = ControlFlow::Exit;
                        }
                        _ => (),
                    }
                }
            }

            _ => {}
        }
    })
}
