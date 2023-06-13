use std::{array, time::Duration};

use color_eyre::Result;
use half::f16;
use rand::Rng;
use voidin::*;

const WIDTH: usize = 640;
const HEIGHT: usize = 640;
type Pixel = [f16; 4];
const PIXEL_SIZE: usize = std::mem::size_of::<Pixel>();

#[derive(Clone, Copy, Default, Debug)]
struct Ray {
    orig: Vec3,
    dir: Vec3,
    t: f32,
}

impl Ray {
    fn new(orig: Vec3, dir: Vec3, t: f32) -> Self {
        Self { orig, dir, t }
    }

    fn intersect(&self, Trig { v0, v1, v2 }: Trig) -> Option<f32> {
        const EPS: f32 = 0.0001;
        let (edge1, edge2) = (v1 - v0, v2 - v0);
        let h = self.dir.cross(edge2);
        let a = edge1.dot(h);
        if -EPS < a && a < EPS {
            return None;
        }
        let f = 1. / a;
        let s = self.orig - v0;
        let u = f * s.dot(h);
        if u < 0. || u > 1. {
            return None;
        }
        let q = s.cross(edge1);
        let v = f * self.dir.dot(q);
        if v < 0. || u + v > 1. {
            return None;
        }
        let t = f * edge2.dot(q);
        match t > EPS {
            true => Some(t.min(self.t)),
            false => None,
        }
    }
}

#[derive(Clone, Copy, Default, Debug)]
struct Trig {
    v0: Vec3,
    v1: Vec3,
    v2: Vec3,
}

struct Triangle {
    triangles: Vec<Trig>,
    cpu_pixels: Vec<Pixel>,
    gpu_pixels: wgpu::Buffer,
}

impl Example for Triangle {
    fn name() -> &'static str {
        "Bvh CPU"
    }

    fn init(app: &mut App) -> Result<Self> {
        let cpu_pixels = vec![[f16::ZERO; 4]; WIDTH * HEIGHT];
        let gpu_pixels = app.device().create_buffer(&wgpu::BufferDescriptor {
            label: Some("Pixels"),
            size: (WIDTH * HEIGHT * PIXEL_SIZE) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let mut rng = rand::thread_rng();
        let mut triangles = vec![Trig::default(); 64];
        for trig in triangles.iter_mut() {
            let v0 = Vec3::from_array(array::from_fn(|_| rng.gen_range(0. ..1.)));
            let v1 = Vec3::from_array(array::from_fn(|_| rng.gen_range(0. ..1.)));
            let v2 = Vec3::from_array(array::from_fn(|_| rng.gen_range(0. ..1.)));
            trig.v0 = v0 * 9. - vec3(5., 5., 5.);
            trig.v1 = trig.v0 + v1;
            trig.v2 = trig.v0 + v2;
        }
        Ok(Self {
            triangles,
            cpu_pixels,
            gpu_pixels,
        })
    }

    fn update(&mut self, _ctx: UpdateContext) {}

    fn resize(&mut self, _gpu: &Gpu, _width: u32, _height: u32) {}

    fn render(&mut self, mut ctx: RenderContext) {
        let cam_pos = vec3(0., 0., -18.);
        let p0 = vec3(-1., 1., -15.);
        let p1 = vec3(1., 1., -15.);
        let p2 = vec3(-1., -1., -15.);
        for (i, p) in self.cpu_pixels.iter_mut().enumerate() {
            let x = (i % WIDTH) as f32 / WIDTH as f32;
            let y = (i / HEIGHT) as f32 / HEIGHT as f32;

            let pixel_pos = p0 + (p1 - p0) * x + (p2 - p0) * y;
            let ray = Ray::new(cam_pos, (pixel_pos - cam_pos).normalize(), 1e30);
            let hit = self.triangles.iter().fold(None, |hit, &trig| {
                let new_hit = ray.intersect(trig);
                match (hit, new_hit) {
                    (Some(a), Some(b)) => Some(f32::min(a, b)),
                    (None, x @ Some(_)) => x,
                    (x @ Some(_), None) => x,
                    (None, None) => None,
                }
            });

            let val = match hit {
                Some(_) => f16::ONE,
                None => f16::ZERO,
            };
            *p = [val, val, val, f16::ONE];
        }

        ctx.gpu
            .queue()
            .write_buffer(&self.gpu_pixels, 0, bytemuck::cast_slice(&self.cpu_pixels));
        ctx.encoder.copy_buffer_to_texture(
            wgpu::ImageCopyBuffer {
                buffer: &self.gpu_pixels,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some((WIDTH * PIXEL_SIZE) as _),
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
