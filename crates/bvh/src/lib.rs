use bytemuck::{Pod, Zeroable};
use glam::{UVec3, UVec4, Vec3, Vec4};

mod tlas;

mod intersection;
use intersection::intersect_aabb;
pub use intersection::{Dist, Ray};
use Dist::*;

const MAX_DIST: f32 = 1e30;

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, Pod, Zeroable)]
pub struct BvhNode {
    pub min: Vec3,
    pub left_first: u32,
    pub max: Vec3,
    pub count: u32,
}

impl BvhNode {
    pub fn triangle_count(&self) -> usize {
        self.count as usize
    }

    pub fn triangle_start(&self) -> usize {
        self.left_first as usize
    }

    pub fn left_node_index(&self) -> usize {
        self.left_first as usize
    }

    pub fn right_node_index(&self) -> usize {
        self.left_first as usize + 1
    }

    pub fn is_leaf(&self) -> bool {
        self.count > 0
    }
}

#[derive(Copy, Clone)]
pub struct Aabb {
    min: Vec3,
    max: Vec3,
}

impl Aabb {
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    fn area(&self) -> f32 {
        let diff = self.max - self.min;
        (diff.x * diff.y + diff.x * diff.z + diff.y * diff.z) * 2.
    }
}

pub struct BvhBuilder<'a> {
    num_bins: usize,
    vertices: &'a [Vec3],
    indices: &'a mut [UVec3],
    centroids: Vec<Vec3>,
    nodes: Vec<BvhNode>,
    triangle_indices: Vec<usize>,
}

impl<'a> BvhBuilder<'a> {
    pub fn new(vertices: &'a [Vec3], indices: &'a mut [UVec3]) -> Self {
        let nodes = vec![BvhNode::default(); indices.len() * 2];

        Self {
            num_bins: 8,
            vertices,
            indices,
            centroids: vec![],
            nodes,
            triangle_indices: vec![],
        }
    }

    pub fn set_bin_number(mut self, num_bins: usize) -> Self {
        self.num_bins = num_bins;
        self
    }

    pub fn build(mut self) -> Bvh {
        self.centroids = self
            .indices
            .iter()
            .map(|idx| {
                [
                    self.vertices[idx[0] as usize],
                    self.vertices[idx[1] as usize],
                    self.vertices[idx[2] as usize],
                ]
            })
            .map(|trig| (trig[0] + trig[1] + trig[2]) / 3f32)
            .collect();

        self.triangle_indices = (0..self.indices.len()).collect();
        self.nodes[0].left_first = 0;
        self.nodes[0].count = self.triangle_indices.len() as u32;

        let aabb = self.calculate_bounds(0, self.nodes[0].count, false);
        self.set_bound(0, &aabb);

        let mut new_node_index = 2;

        self.subdivide(0, 0, &mut new_node_index);
        self.nodes.truncate(new_node_index as usize);

        let indices_copy: Vec<_> = self
            .triangle_indices
            .into_iter()
            .map(|i| self.indices[i])
            .collect();
        self.indices.copy_from_slice(&indices_copy);

        Bvh { nodes: self.nodes }
    }

    fn subdivide(&mut self, current_bvh_index: usize, start: u32, pool_index: &mut u32) {
        if self.nodes[current_bvh_index].count <= 3 {
            self.nodes[current_bvh_index].left_first = start;
            return;
        }
        let index = *pool_index;
        *pool_index += 2;
        self.nodes[current_bvh_index].left_first = index;

        let pivot = self.partition(start, self.nodes[current_bvh_index].count);
        let left_count = pivot - start;
        self.nodes[index as usize].count = left_count;
        let bounds = self.calculate_bounds(start, left_count, false);
        self.set_bound(index as usize, &bounds);

        let right_count = self.nodes[current_bvh_index].count - left_count;
        self.nodes[index as usize + 1].count = right_count;
        let bounds = self.calculate_bounds(pivot, right_count, false);
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
            for scale in (1..bins).map(|b| (b as f32) / (bins as f32)) {
                let pos = aabb.min.lerp(aabb.max, scale)[axis];
                let pivot = self.partition_shuffle(axis, pos, start, count);

                let bb1_count = pivot - start;
                let bb2_count = count - bb1_count;

                let bb1 = self.calculate_bounds(start, bb1_count, false);
                let bb2 = self.calculate_bounds(pivot, bb2_count, false);

                let cost = bb1.area() * bb1_count as f32 + bb2.area() * bb2_count as f32;
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
        let mut end = (start + count - 1) as usize;
        let mut i = start as usize;

        while i < end {
            if self.centroids[self.triangle_indices[i]][axis] < pos {
                i += 1;
            } else {
                self.triangle_indices.swap(i, end);
                end -= 1;
            }
        }

        i as u32
    }

    fn calculate_bounds(&self, first: u32, amount: u32, centroids: bool) -> Aabb {
        let mut max = Vec3::splat(-MAX_DIST);
        let mut min = Vec3::splat(MAX_DIST);
        for &idx in &self.triangle_indices[first as usize..][..amount as usize] {
            if centroids {
                let vertex = self.centroids[idx];
                max = max.max(vertex);
                min = min.min(vertex);
            } else {
                self.indices[idx].to_array()[..3]
                    .iter()
                    .map(|&i| self.vertices[i as usize])
                    .for_each(|vertex| {
                        max = max.max(vertex);
                        min = min.min(vertex);
                    });
            }
        }
        Aabb { max, min }
    }
}

pub struct Bvh {
    pub nodes: Vec<BvhNode>,
}

impl Bvh {
    pub fn traverse(
        &self,
        vertices: &[Vec4],
        indices: &[UVec4],
        ray: Ray,
        node_idx: usize,
        mut t: f32,
    ) -> Dist {
        let node = &self.nodes[node_idx];
        let Hit(_) = intersect_aabb(ray, node.min, node.max, t) else {
            return Miss;
        };
        if node.is_leaf() {
            for i in 0..node.triangle_count() {
                let idx = indices[node.triangle_start() + i];
                let trig = [
                    vertices[idx[0] as usize].truncate(),
                    vertices[idx[1] as usize].truncate(),
                    vertices[idx[2] as usize].truncate(),
                ];
                if let Hit(dist) = ray.intersect(trig) {
                    t = t.min(dist);
                }
            }
            return Hit(t);
        } else {
            if let Hit(dist) = self.traverse(vertices, indices, ray, node.left_node_index(), t) {
                t = t.min(dist);
            }
            if let Hit(dist) = self.traverse(vertices, indices, ray, node.right_node_index(), t) {
                t = t.min(dist);
            }
        }
        Hit(t)
    }

    pub fn traverse_iter(&self, vertices: &[Vec4], indices: &[UVec4], ray: Ray) -> Dist {
        let mut stack = Stack::new();
        stack.push(0);

        let mut hit = Dist::Miss;
        while !stack.is_empty() {
            let node = self.nodes[stack.pop()];
            if node.is_leaf() {
                for i in 0..node.triangle_count() {
                    let idx = indices[node.triangle_start() + i];
                    let trig = [
                        vertices[idx[0] as usize].truncate(),
                        vertices[idx[1] as usize].truncate(),
                        vertices[idx[2] as usize].truncate(),
                    ];
                    if let Hit(dist) = ray.intersect(trig) {
                        hit = match hit {
                            Hit(t) => Hit(t.min(dist)),
                            Miss => Hit(dist),
                        }
                    }
                }
            } else {
                let mut min_index = node.left_node_index();
                let mut max_index = node.right_node_index();

                let min_child = self.nodes[min_index];
                let max_child = self.nodes[max_index];

                let mut min_dist =
                    intersect_aabb(ray, min_child.min, min_child.max, hit.unwrap_or(MAX_DIST));
                let mut max_dist =
                    intersect_aabb(ray, max_child.min, max_child.max, hit.unwrap_or(MAX_DIST));
                if min_dist > max_dist {
                    (min_index, max_index) = (max_index, min_index);
                    (min_dist, max_dist) = (max_dist, min_dist);
                }

                match min_dist {
                    Hit(_) => stack.push(min_index),
                    Miss => continue,
                }
                if let Hit(_) = max_dist {
                    stack.push(max_index);
                }
            }
        }
        hit
    }
}

struct Stack {
    arr: [usize; 32],
    head: usize,
}

impl Stack {
    fn new() -> Self {
        Self {
            arr: [usize::MAX; 32],
            head: 0,
        }
    }

    fn is_empty(&self) -> bool {
        self.head == 0
    }

    fn push(&mut self, val: usize) {
        self.arr[self.head] = val;
        self.head += 1;
    }

    fn pop(&mut self) -> usize {
        self.head -= 1;
        self.arr[self.head]
    }
}
