use std::{array, fmt::Debug, time::Duration};

use color_eyre::Result;
use glam::Vec4Swizzles;
use half::f16;
use rand::Rng;
use voidin::*;

const WIDTH: usize = 640;
const HEIGHT: usize = 640;
type Pixel = [f16; 4];
const PIXEL_SIZE: usize = std::mem::size_of::<Pixel>();

const MAX_DIST: f32 = 1e30;

#[derive(PartialOrd, PartialEq)]
enum Dist {
    Hit(f32),
    Miss,
}
use Dist::*;

impl From<Option<f32>> for Dist {
    fn from(value: Option<f32>) -> Self {
        match value {
            Some(tee) => Self::Hit(tee),
            None => Self::Miss,
        }
    }
}

#[derive(Clone, Copy, Default, Debug)]
struct Ray {
    orig: Vec3,
    dir: Vec3,
}

impl Ray {
    fn new(orig: Vec3, dir: Vec3) -> Self {
        Self { orig, dir }
    }

    fn intersect(&self, Trig([v0, v1, v2]): Trig) -> Dist {
        const EPS: f32 = 0.0001;
        let (edge1, edge2) = (v1 - v0, v2 - v0);
        let h = self.dir.cross(edge2);
        let a = edge1.dot(h);
        if -EPS < a && a < EPS {
            return Miss;
        }
        let f = 1. / a;
        let s = self.orig - v0;
        let u = f * s.dot(h);
        if !(0. ..=1.).contains(&u) {
            return Miss;
        }
        let q = s.cross(edge1);
        let v = f * self.dir.dot(q);
        if v < 0. || u + v > 1. {
            return Miss;
        }
        let t = f * edge2.dot(q);
        match t > EPS {
            true => Hit(t),
            false => Miss,
        }
    }
}

#[derive(Clone, Copy, Default, Debug)]
struct Trig([Vec3; 3]);

impl Trig {
    fn new(v0: Vec3, v1: Vec3, v2: Vec3) -> Self {
        Self([v0, v1, v2])
    }
}

struct Demo {
    cpu_pixels: Vec<Pixel>,
    gpu_pixels: wgpu::Buffer,

    bvh: Bvh,
}

impl Example for Demo {
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
            let v0 = Vec3::from(array::from_fn(|_| rng.gen_range(0. ..1.)));
            let v1 = Vec3::from(array::from_fn(|_| rng.gen_range(0. ..1.)));
            let v2 = Vec3::from(array::from_fn(|_| rng.gen_range(0. ..1.)));
            let base = v0 * 9. - vec3(5., 5., 0.);
            *trig = Trig::new(base, base + v1, base + v2);
        }

        let mut bvh = Bvh::new(&triangles);
        bvh.build_bvh();

        Ok(Self {
            cpu_pixels,
            gpu_pixels,

            bvh,
        })
    }

    fn update(&mut self, _ctx: UpdateContext) {}

    fn resize(&mut self, _gpu: &Gpu, _width: u32, _height: u32) {}

    fn render(&mut self, mut ctx: RenderContext) {
        let camera = ctx.app_state.camera.get_uniform(None, None);
        for (i, p) in self.cpu_pixels.iter_mut().enumerate() {
            let x = (i % WIDTH) as f32 / WIDTH as f32;
            let y = (i / HEIGHT) as f32 / HEIGHT as f32;
            let Vec2 { x, y } = (vec2(x, y) - 0.5) * vec2(2., -2.);

            let view_pos = camera.clip_to_world * vec4(x, y, 1., 1.);
            let view_tang = camera.clip_to_world * vec4(x, y, 0., 1.);

            let eye = view_pos.xyz() / view_pos.w;
            let dir = view_tang.xyz().normalize();

            let ray = Ray::new(eye, dir);

            // let hit = self.bvh.traverse(ray, 0, MAX_DIST);
            let hit = self.bvh.traverse_iter(ray);
            let val = match hit {
                Hit(dist) => {
                    let limit = 50.;
                    f16::from_f32((limit - dist) / limit)
                }
                Miss => f16::ZERO,
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

    let camera = Camera::new(vec3(0., 0., 15.), 0., 0.);
    run::<Demo>(window, camera)
}

#[derive(Copy, Clone, Default)]
pub struct BVHNode {
    min: Vec3,
    max: Vec3,
    pub left_first: i32,
    pub count: i32,
}

#[derive(Copy, Clone)]
pub struct Aabb {
    min: Vec3,
    max: Vec3,
}

impl Aabb {
    fn area(&self) -> f32 {
        let diff = self.max - self.min;
        (diff.x * diff.y + diff.x * diff.z + diff.y * diff.z) * 2.
    }
}

pub struct Bvh {
    pub triangles: Vec<[Vec3; 3]>,
    pub indices: Vec<u32>,
    pub nodes: Vec<BVHNode>,
    pub centroids: Vec<Vec3>,
}

impl BVHNode {
    fn is_leaf(&self) -> bool {
        self.count > 0
    }
}

fn intersect_aabb(ray: Ray, bmin: Vec3, bmax: Vec3, t: f32) -> Dist {
    let tx1 = (bmin - ray.orig) / ray.dir;
    let tx2 = (bmax - ray.orig) / ray.dir;
    let tmax = tx1.max(tx2).min_element();
    let tmin = tx1.min(tx2).max_element();
    (tmax >= tmin && tmin < t && tmax > 0.)
        .then_some(tmin)
        .into()
}

impl Bvh {
    fn new(triangles: &[Trig]) -> Bvh {
        let triangles: Vec<_> = triangles.iter().map(|t| t.0).collect();

        let indices: Vec<u32> = (0..triangles.len() as u32).collect();

        let bvh_nodes = vec![BVHNode::default(); triangles.len() * 2];

        Bvh {
            triangles,
            indices,
            nodes: bvh_nodes,
            centroids: Default::default(),
        }
    }

    #[allow(dead_code)]
    fn traverse(&self, ray: Ray, node_idx: usize, mut t: f32) -> Dist {
        let node = &self.nodes[node_idx];
        let Hit(_) = intersect_aabb(ray, node.min , node.max, t) else { return Miss };
        if node.is_leaf() {
            for i in 0..node.count as usize {
                let [v0, v1, v2] = self.triangles[node.left_first as usize + i];
                let trig = Trig::new(v0, v1, v2);
                if let Hit(dist) = ray.intersect(trig) {
                    t = t.min(dist);
                }
            }
            return Hit(t);
        } else {
            if let Hit(dist) = self.traverse(ray, node.left_first as _, t) {
                t = t.min(dist);
            }
            if let Hit(dist) = self.traverse(ray, node.left_first as usize + 1, t) {
                t = t.min(dist);
            }
        }
        Hit(t)
    }

    fn traverse_iter(&self, ray: Ray) -> Dist {
        #[derive(Default, Clone, Copy)]
        struct StackNode {
            node_idx: usize,
            dist: f32,
        }
        let mut node_idx = 0;
        let mut stack = [StackNode::default(); 64];
        let mut stack_ptr = 0;
        let mut ray_t = MAX_DIST;
        loop {
            let node = self.nodes[node_idx];
            if node.is_leaf() {
                for i in 0..node.count as usize {
                    let [v0, v1, v2] = self.triangles[node.left_first as usize + i];
                    if let Hit(dist) = ray.intersect(Trig::new(v0, v1, v2)) {
                        ray_t = ray_t.min(dist);
                    }
                }

                if stack_ptr == 0 {
                    break Hit(ray_t);
                } else {
                    let mut t = MAX_DIST;
                    while t >= ray_t {
                        if stack_ptr == 0 {
                            return Hit(ray_t);
                        }
                        stack_ptr -= 1;
                        let snode = stack[stack_ptr];
                        t = snode.dist;
                        node_idx = snode.node_idx;
                    }
                    continue;
                }
            }

            let mut child_idx1 = node.left_first as usize;
            let mut child_idx2 = node.left_first as usize + 1;

            let child1 = self.nodes[child_idx1];
            let child2 = self.nodes[child_idx2];
            let mut dist1 = intersect_aabb(ray, child1.min, child1.max, ray_t);
            let mut dist2 = intersect_aabb(ray, child2.min, child2.max, ray_t);
            if dist1 > dist2 {
                (dist1, dist2) = (dist2, dist1);
                (child_idx1, child_idx2) = (child_idx2, child_idx1);
            }
            if matches!(dist1, Hit(_)) {
                node_idx = child_idx1;
                if let Hit(dist2) = dist2 {
                    stack[stack_ptr].node_idx = child_idx2;
                    stack[stack_ptr].dist = dist2;
                    stack_ptr += 1;
                }
            } else if stack_ptr == 0 {
                return Miss;
            } else {
                let mut t = MAX_DIST;
                while t >= ray_t {
                    if stack_ptr == 0 {
                        return Hit(ray_t);
                    }
                    stack_ptr -= 1;
                    let snode = stack[stack_ptr];
                    t = snode.dist;
                    node_idx = snode.node_idx;
                }
            }
        }
    }

    pub fn build_bvh(&mut self) {
        self.centroids = self
            .triangles
            .iter()
            .map(|t| (t[0] + t[1] + t[2]) / 3f32)
            .collect();

        self.nodes[0].left_first = 0;
        self.nodes[0].count = self.triangles.len() as i32;

        let aabb = self.calculate_bounds(0, self.triangles.len() as u32, false);
        self.set_bound(0, &aabb);

        let mut new_node_index = 2;

        self.subdivide(0, 0, &mut new_node_index);
        self.nodes.truncate(new_node_index as usize);

        self.triangles = self
            .indices
            .iter()
            .map(|index| self.triangles[*index as usize])
            .collect();
    }

    fn subdivide(&mut self, current_bvh_index: usize, start: u32, pool_index: &mut u32) {
        if self.nodes[current_bvh_index].count <= 3 {
            self.nodes[current_bvh_index].left_first = start as i32;
            return;
        }
        let index = *pool_index;
        *pool_index += 2;
        self.nodes[current_bvh_index].left_first = index as i32;

        let pivot = self.partition(start, self.nodes[current_bvh_index].count as u32);
        let left_count = pivot - start;
        self.nodes[index as usize].count = left_count as i32;
        let bounds = self.calculate_bounds(start, left_count, false);
        self.set_bound(index as usize, &bounds);

        let right_count = self.nodes[current_bvh_index].count - left_count as i32;
        self.nodes[index as usize + 1].count = right_count;
        let bounds = self.calculate_bounds(pivot, right_count as u32, false);
        self.set_bound(index as usize + 1, &bounds);

        self.subdivide(index as usize, start, pool_index);
        self.subdivide(index as usize + 1, pivot, pool_index);
        self.nodes[current_bvh_index].count = 0;
    }

    fn set_bound(&mut self, bvh_index: usize, aabb: &Aabb) {
        self.nodes[bvh_index].max = aabb.max;
        self.nodes[bvh_index].min = aabb.min;
    }

    fn partition(&mut self, start: u32, count: u32) -> u32 {
        let bins = 8;
        let mut optimal_axis = 0;
        let mut optimal_pos = 0f32;
        let mut optimal_pivot = 0;
        let mut optimal_cost = f32::MAX;

        let aabb = self.calculate_bounds(start, count, true);

        for axis in 0..3 {
            for b in 1..bins {
                let pos = aabb.min.lerp(aabb.max, (b as f32) / (bins as f32))[axis];
                let pivot = self.partition_shuffle(axis, pos, start, count);

                let bb1_count = pivot - start;
                let bb2_count = count - bb1_count;

                let bb1 = self.calculate_bounds(start, bb1_count, false);
                let bb2 = self.calculate_bounds(pivot, bb2_count, false);

                let half_area1 = bb1.area();
                let half_area2 = bb2.area();

                let cost = half_area1 * bb1_count as f32 + half_area2 * bb2_count as f32;
                if cost < optimal_cost {
                    optimal_axis = axis;
                    optimal_pos = pos;
                    optimal_pivot = pivot;
                    optimal_cost = cost;
                }
            }
        }
        self.partition_shuffle(optimal_axis, optimal_pos, start, count);
        optimal_pivot
    }

    fn partition_shuffle(&mut self, axis: usize, pos: f32, start: u32, count: u32) -> u32 {
        let mut end = (start + count - 1) as i32;
        let mut i = start as i32;

        while i < end {
            if self.centroids[self.indices[i as usize] as usize][axis] < pos {
                i += 1;
            } else {
                self.indices.swap(i as usize, end as usize);
                end -= 1;
            }
        }

        i as u32
    }

    fn calculate_bounds(&self, first: u32, amount: u32, centroids: bool) -> Aabb {
        let mut max = Vec3::splat(-MAX_DIST);
        let mut min = Vec3::splat(MAX_DIST);
        for &idx in &self.indices[first as usize..][..amount as usize] {
            if centroids {
                let vertex = self.centroids[idx as usize];
                max = max.max(vertex);
                min = min.min(vertex);
            } else {
                self.triangles[idx as usize].iter().for_each(|&vertex| {
                    max = max.max(vertex);
                    min = min.min(vertex);
                });
            }
        }
        Aabb { max, min }
    }
}
