#import <shared.wgsl>
#import <utils/math.wgsl>

const STACK_LEN: u32 = 32u;
struct Stack {
    arr: array<u32, STACK_LEN>,
	head: u32,
}

fn stack_new() -> Stack {
    var arr: array<u32, STACK_LEN>;
    for (var i = 0u; i < STACK_LEN; i += 1u) {
        arr[i] = 0u;
    }
    return Stack(arr, 0u);
}

fn stack_push(stack: ptr<function, Stack>, val: u32) {
    (*stack).arr[(*stack).head] = val;
    (*stack).head += 1u;
}

fn stack_pop(stack: ptr<function, Stack>) -> u32 {
    (*stack).head -= 1u;
    return (*stack).arr[(*stack).head];
}

struct Ray {
    eye: vec3<f32>,
	dir: vec3<f32>,
	inv_dir: vec3<f32>,
	t: f32,
}

fn ray_new(eye: vec3<f32>, dir: vec3<f32>) -> Ray {
    return Ray(eye, dir, 1. / dir, MAX_DIST);
}

fn intersect_aabb(ray: Ray, bmin: vec3<f32>, bmax: vec3<f32>, t: f32) -> f32 {
    let tx1 = (bmin - ray.eye) * ray.inv_dir;
    let tx2 = (bmax - ray.eye) * ray.inv_dir;
    let tmax = min_element(max(tx1, tx2));
    let tmin = max_element(min(tx1, tx2));
    if tmax >= tmin && tmin < t && tmax > 0. {
        return tmin;
    } else {
        return MAX_DIST;
    }
}

fn intersect_trig(ray: Ray, v0: vec3<f32>, v1: vec3<f32>, v2: vec3<f32>) -> f32 {
    let edge1 = v1 - v0;
    let edge2 = v2 - v0;
    let uvec = cross(ray.dir, edge2);
    let det = dot(edge1, uvec);
    if -EPS < det && det < EPS { return MAX_DIST; }
    let inv_det = 1. / det;
    let orig = ray.eye - v0;
    let u = inv_det * dot(orig, uvec);
    if u < 0. || 1. < u { return MAX_DIST; }
    let vvec = cross(orig, edge1);
    let v = inv_det * dot(ray.dir, vvec);
    if v < 0. || u + v > 1. { return MAX_DIST; }
    let t = inv_det * dot(edge2, vvec);
    if t > EPS {
        return t;
    } else {
        return MAX_DIST;
    }
}

struct BvhNode {
	min: vec3<f32>,
	left_first: u32,
	max: vec3<f32>,
	count: u32,
}

fn traverse_bvh(ray: Ray) -> f32 {
    var stack = stack_new();
    stack_push(&stack, 0u);

    var hit = MAX_DIST;
    while stack.head != 0u {
        let node = nodes[stack_pop(&stack)];
		// is leaf
        if node.count > 0u {
            for (var i = 0u; i < node.count; i += 1u) {
                let idx = indices[3u * (node.left_first + i)];
                let v0 = vertices[ idx + 0u ];
                let v1 = vertices[ idx + 1u ];
                let v2 = vertices[ idx + 2u ];
                let t = intersect_trig(ray, v0, v1, v2);
                hit = min(hit, t);
            }
        } else {
            var min_index = node.left_first;
            var max_index = node.left_first + 1u;

            let min_child = nodes[min_index];
            let max_child = nodes[max_index];

            var min_dist = intersect_aabb(ray, min_child.min, min_child.max, hit);
            var max_dist = intersect_aabb(ray, max_child.min, max_child.max, hit);
            if min_dist > max_dist {
                var tmpd = min_dist;
                max_dist = min_dist;
                min_dist = tmpd;
                var tmpi = min_index;
                max_index = min_index;
                min_index = tmpi;
            }

            if min_dist < MAX_DIST {
                stack_push(&stack, min_index);
            } else {
				continue;
            }
            if max_dist < MAX_DIST {
                stack_push(&stack, max_index);
            }
        }
    }

    return hit;
}

@group(0) @binding(0) var<uniform> cam: Camera;

@group(1) @binding(0) var<storage, read> vertices: array<vec3<f32>>;
@group(1) @binding(1) var<storage, read> indices: array<u32>;
@group(1) @binding(2) var<storage, read> nodes: array<BvhNode>;

struct VertexOutput {
	@builtin(position) pos: vec4<f32>,
	@location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_idx: u32) -> VertexOutput {
    var out: VertexOutput;
    out.uv = vec2<f32>(vec2((vertex_idx << 1u) & 2u, vertex_idx & 2u));
    out.pos = vec4(2.0 * out.uv - 1.0, 0.0, 1.0);
    // out.pos = vec4(2.0 * out.uv.x - 1.0, 1.0 - out.uv.y * 2.0, 0.0, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv - 0.5;

    let view_pos = cam.clip_to_world * vec4(uv, 1., 1.);
    let view_dir = cam.clip_to_world * vec4(uv, 0., 1.);

    let eye = view_pos.xyz / view_pos.w;
    let dir = normalize(view_dir.xyz);

    let ray = ray_new(eye, dir);

    var color = vec3(0.13);
    let t = traverse_bvh(ray);
    if t < MAX_DIST {
        let limit = 50.;
        color = vec3((limit - t) / limit);
    }

    return vec4(color, 1.0);
}
