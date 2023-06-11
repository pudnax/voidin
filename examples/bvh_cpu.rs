use std::time::Duration;

use color_eyre::Result;
use half::f16;
use voidin::*;

struct Triangle {
    cpu_pixels: Vec<[f16; 4]>,
    gpu_pixels: wgpu::Buffer,
}

const WIDTH: usize = 640;
const HEIGHT: usize = 640;

impl Example for Triangle {
    fn name() -> &'static str {
        "Bvh CPU"
    }

    fn init(app: &mut App) -> Result<Self> {
        let cpu_pixels = vec![[f16::ZERO; 4]; WIDTH * HEIGHT];
        let gpu_pixels = app.device().create_buffer(&wgpu::BufferDescriptor {
            label: Some("Pixels"),
            size: (WIDTH * HEIGHT * std::mem::size_of::<[f16; 4]>()) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        Ok(Self {
            cpu_pixels,
            gpu_pixels,
        })
    }

    fn update(&mut self, _ctx: UpdateContext) {}

    fn resize(&mut self, _gpu: &Gpu, _width: u32, _height: u32) {}

    fn render(&mut self, mut ctx: RenderContext) {
        for (i, p) in self.cpu_pixels.iter_mut().enumerate() {
            let x = (i % WIDTH) as f32 / WIDTH as f32;
            let y = (i / HEIGHT) as f32 / HEIGHT as f32;
            *p = [f16::from_f32(x), f16::from_f32(y), f16::ZERO, f16::ONE];
        }
        ctx.gpu
            .queue()
            .write_buffer(&self.gpu_pixels, 0, bytemuck::cast_slice(&self.cpu_pixels));

        ctx.encoder.copy_buffer_to_texture(
            wgpu::ImageCopyBuffer {
                buffer: &self.gpu_pixels,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some((WIDTH * std::mem::size_of::<[f16; 4]>()) as _),
                    rows_per_image: None,
                },
            },
            ctx.view_target.main_texture().as_image_copy(),
            wgpu::Extent3d {
                width: WIDTH as _,
                height: HEIGHT as _,
                depth_or_array_layers: 1,
            },
        );

        ctx.ui(|egui_ctx| {
            egui::Window::new("debug").show(egui_ctx, |ui| {
                ui.label(format!(
                    "Fps: {:.04?}",
                    Duration::from_secs_f64(ctx.app_state.dt)
                ));
            });
        });
    }
}

fn main() -> Result<()> {
    let window = WindowBuilder::new()
        .with_inner_size(LogicalSize::new(WIDTH as u32, HEIGHT as u32))
        .with_resizable(false);

    let camera = Camera::new(vec3(0., 0., 0.), 0., 0.);
    run::<Triangle>(window, camera)
}
