use glam::{vec2, Vec2};
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{
        DeviceEvent, ElementState, Event, KeyboardInput, MouseScrollDelta, VirtualKeyCode,
        WindowEvent,
    },
    window::Window,
};

#[derive(Debug, Default)]
pub struct Input {
    pub q_pressed: bool,
    pub e_pressed: bool,
    pub up_pressed: bool,
    pub down_pressed: bool,
    pub right_pressed: bool,
    pub left_pressed: bool,
    pub shift_pressed: bool,
    pub ctrl_pressed: bool,
    pub enter_pressed: bool,
    pub space_pressed: bool,
    pub left_mouse_pressed: bool,
    pub mouse_position: Vec2,
    pub mouse_delta: Vec2,
    pub mouse_scroll: f32,
}

impl Input {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn update<T>(&mut self, event: &Event<'_, T>, window: &Window) -> bool {
        match event {
            Event::DeviceEvent { event, .. } => match event {
                DeviceEvent::MouseWheel { delta, .. } => {
                    self.mouse_scroll = -match delta {
                        MouseScrollDelta::LineDelta(_, scroll) => *scroll,
                        MouseScrollDelta::PixelDelta(PhysicalPosition { y: scroll, .. }) => {
                            *scroll as f32
                        }
                    };
                }
                DeviceEvent::MouseMotion { delta: (dx, dy) } => {
                    self.mouse_delta = vec2(*dx as _, *dy as _);
                }
                _ => {}
            },
            Event::WindowEvent { event, window_id } if *window_id == window.id() => match event {
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            virtual_keycode: Some(keycode),
                            state,
                            ..
                        },
                    ..
                } => {
                    let pressed = state == &ElementState::Pressed;
                    match keycode {
                        VirtualKeyCode::Q => {
                            self.q_pressed = pressed;
                        }
                        VirtualKeyCode::E => {
                            self.e_pressed = pressed;
                        }
                        VirtualKeyCode::Up | VirtualKeyCode::W => {
                            self.up_pressed = pressed;
                        }
                        VirtualKeyCode::Down | VirtualKeyCode::S => {
                            self.down_pressed = pressed;
                        }
                        VirtualKeyCode::Left | VirtualKeyCode::A => {
                            self.left_pressed = pressed;
                        }
                        VirtualKeyCode::Right | VirtualKeyCode::D => {
                            self.right_pressed = pressed;
                        }
                        VirtualKeyCode::RShift | VirtualKeyCode::LShift => {
                            self.shift_pressed = pressed;
                        }
                        VirtualKeyCode::RControl | VirtualKeyCode::LControl => {
                            self.ctrl_pressed = pressed;
                        }
                        VirtualKeyCode::Return => {
                            self.enter_pressed = pressed;
                        }
                        VirtualKeyCode::Space => {
                            self.space_pressed = pressed;
                        }
                        _ => return false,
                    };
                }
                WindowEvent::CursorMoved {
                    position: PhysicalPosition { x, y },
                    ..
                } => {
                    let PhysicalSize { width, height } = window.inner_size();
                    let x = (*x as f32 / width as f32 - 0.5) * 2.;
                    let y = -(*y as f32 / height as f32 - 0.5) * 2.;
                    self.mouse_position = vec2(x, y);
                }
                WindowEvent::MouseInput {
                    button: winit::event::MouseButton::Left,
                    state,
                    ..
                } => self.left_mouse_pressed = matches!(state, ElementState::Pressed),
                _ => {}
            },

            _ => {}
        }
        true
    }
}
