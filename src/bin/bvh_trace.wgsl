#import <shared.wgsl>
#import <utils/math.wgsl>

const STACK_LEN: u32 = 32u;
struct Stack {
    arr: array<u32, STACK_LEN>,
	head: u32,
}

fn stack_new() -> Stack {
    var arr: array<u32, STACK_LEN>;
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
}

fn ray_new(eye: vec3<f32>, dir: vec3<f32>) -> Ray {
    return Ray(eye, dir, 1. / dir);
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

fn intersect_trig(ray: Ray, v0: vec3<f32>, v1: vec3<f32>, v2: vec3<f32>, hit: ptr<function,f32>) -> bool {
    let edge1 = v1 - v0;
    let edge2 = v2 - v0;
    let uvec = cross(ray.dir, edge2);
    let det = dot(edge1, uvec);
    if det < 1e-10 { return false; } // cull backface
    let inv_det = 1. / det;
    let orig = ray.eye - v0;
    let u = inv_det * dot(orig, uvec);
    if u < 0. || 1. < u { return false; }
    let vvec = cross(orig, edge1);
    let v = inv_det * dot(ray.dir, vvec);
    if v < 0. || u + v > 1. { return false; }
    let t = inv_det * dot(edge2, vvec);
    if t > 0.0 && t < *hit {
        *hit = t;
        return true;
    } else {
        return false;
    }
}

fn triangle_normal(v0: vec3<f32>, v1: vec3<f32>, v2: vec3<f32>) -> vec3<f32> {
    let  p1 = v1 - v0;
    let p2 = v1 - v2;
    return normalize(cross(normalize(p1), normalize(p2)));
}

struct TlasNode {
	min: vec3<f32>,
	left_right: u32,
	max: vec3<f32>,
	instance_idx: u32,
}

struct BvhNode {
	min: vec3<f32>,
	left_first: u32,
	max: vec3<f32>,
	count: u32,
}

struct TraceResult {
	v0: vec3<f32>,
	v1: vec3<f32>,
	v2: vec3<f32>,
	hit: bool,
	dist: f32,
}

fn trace_result_new() -> TraceResult {
    return TraceResult(vec3(0.), vec3(0.), vec3(0.), false, MAX_DIST);
}

fn fetch_vertex(idx: u32, vertex_offset: u32) -> vec3<f32> {
    let i = vertex_offset + 3u * indices[idx];
    return vec3(vertices[i + 0u], vertices[i + 1u], vertices[i + 2u]);
}

fn traverse_bvh(ray: Ray, mesh: MeshInfo, res: ptr<function, TraceResult>) {
    var stack = stack_new();
    stack_push(&stack, mesh.bvh_index);

    var hit = (*res).dist;
    while stack.head > 0u {
        let node = bvh_nodes[stack_pop(&stack)];
        if node.count > 0u { // is leaf
            for (var i = 0u; i < node.count; i += 1u) {
                let base_index = node.left_first + i;
                let v0 = fetch_vertex(mesh.base_index + 3u * base_index + 0u, u32(mesh.vertex_offset));
                let v1 = fetch_vertex(mesh.base_index + 3u * base_index + 1u, u32(mesh.vertex_offset));
                let v2 = fetch_vertex(mesh.base_index + 3u * base_index + 2u, u32(mesh.vertex_offset));
                if intersect_trig(ray, v0, v1, v2, &hit) {
                    *res = TraceResult(v0, v1, v2, true, hit);
                }
            }
        } else {
            var min_index = mesh.bvh_index + node.left_first;
            var max_index = mesh.bvh_index + node.left_first + 1u;

            let min_child = bvh_nodes[min_index];
            let max_child = bvh_nodes[max_index];

            var min_dist = intersect_aabb(ray, min_child.min, min_child.max, hit);
            var max_dist = intersect_aabb(ray, max_child.min, max_child.max, hit);
            if min_dist > max_dist {
                swapu(&min_index, &max_index);
                swapf(&min_dist, &max_dist);
            }

            if min_dist >= MAX_DIST {
				 continue;
            }

            if max_dist < MAX_DIST {
                stack_push(&stack, max_index);
            }
            stack_push(&stack, min_index);
        }
    }
}

fn instance_intersect(ray: Ray, instance: Instance, res: ptr<function, TraceResult>) {
    var new_ray = ray;

    let mesh = meshes[instance.mesh_id];
    new_ray.eye = (instance.inv_transform * vec4(ray.eye, 1.)).xyz;
    new_ray.dir = (instance.inv_transform * vec4(ray.dir, 0.)).xyz;
    new_ray.inv_dir = 1. / new_ray.dir;

    traverse_bvh(new_ray, mesh, res);
}

fn traverse_tlas(ray: Ray) -> TraceResult {
    var stack = stack_new();
    stack_push(&stack, 0u);

    var hit = MAX_DIST;
    var res = trace_result_new();
    while stack.head > 0u {
        let node = tlas_nodes[stack_pop(&stack)];
        if node.left_right == 0u { // is leaf
            instance_intersect(ray, instances[node.instance_idx], &res);
		} else {
            var min_index = node.left_right & 0xffffu;
            var max_index = node.left_right >> 16u;

            let min_child = tlas_nodes[min_index];
            let max_child = tlas_nodes[max_index];

            var min_dist = intersect_aabb(ray, min_child.min, min_child.max, hit);
            var max_dist = intersect_aabb(ray, max_child.min, max_child.max, hit);
            if min_dist > max_dist {
                swapu(&min_index, &max_index);
                swapf(&min_dist, &max_dist);
            }

            if min_dist >= MAX_DIST {
				 continue;
            }

            if max_dist < MAX_DIST {
                stack_push(&stack, max_index);
            }
            stack_push(&stack, min_index);
        }
    }
    return res;
}

@group(0) @binding(0) var<uniform> cam: Camera;

@group(1) @binding(0) var<storage, read> tlas_nodes: array<TlasNode>;
@group(1) @binding(1) var<storage, read> instances: array<Instance>;
@group(1) @binding(2) var<storage, read> meshes: array<MeshInfo>;
@group(1) @binding(3) var<storage, read> bvh_nodes: array<BvhNode>;
@group(1) @binding(4) var<storage, read> vertices: array<f32>;
@group(1) @binding(5) var<storage, read> indices: array<u32>;

struct VertexOutput {
	@builtin(position) pos: vec4<f32>,
	@location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_idx: u32) -> VertexOutput {
    var out: VertexOutput;
    out.uv = vec2<f32>(vec2((vertex_idx << 1u) & 2u, vertex_idx & 2u));
    out.pos = vec4(2.0 * out.uv - 1.0, 0.0, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv * 2. - 1.0;

    let view_pos = cam.clip_to_world * vec4(uv, 1., 1.);
    let view_dir = cam.clip_to_world * vec4(uv, 0., 1.);

    let eye = view_pos.xyz / view_pos.w;
    let dir = normalize(view_dir.xyz);

    let ray = ray_new(eye, dir);

    var color = vec3(0.05);
    let res = traverse_tlas(ray);
    if res.hit {
        let nor = triangle_normal(res.v0, res.v1, res.v2);
        color = vec3(length(sin(-nor * 2.5) * 0.5 + 0.5) / sqrt(3.));
    }

    return vec4(color, 1.0);
}
