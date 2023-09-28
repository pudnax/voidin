#import "./stack.wgsl"
#import "./intersections.wgsl"

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

fn fetch_vertex(idx: u32, mesh: MeshInfo) -> vec3<f32> {
    let i = u32(mesh.vertex_offset) + indices[mesh.base_index + idx];
    return vec3(vertices[3u * i + 0u], vertices[3u * i + 1u], vertices[3u * i + 2u]);
}

fn traverse_bvh(ray: Ray, mesh: MeshInfo, res: ptr<function, TraceResult>) {
    var stack = stack_new();
    stack_push(&stack, mesh.bvh_index);

    var hit = (*res).dist;
    while stack.head > 0u {
        let node = bvh_nodes[stack_pop(&stack)];
        if node.count > 0u { // is leaf
            for (var i = 0u; i < node.count; i += 1u) {
                let idx = node.left_first + i;
                let v0 = fetch_vertex(3u * idx + 0u, mesh);
                let v1 = fetch_vertex(3u * idx + 1u, mesh);
                let v2 = fetch_vertex(3u * idx + 2u, mesh);
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

            if min_dist >= hit {
				 continue;
            }

            if max_dist <= hit {
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

            var min_dist = intersect_aabb(ray, min_child.min, min_child.max, res.dist);
            var max_dist = intersect_aabb(ray, max_child.min, max_child.max, res.dist);
            if min_dist > max_dist {
                swapu(&min_index, &max_index);
                swapf(&min_dist, &max_dist);
            }

            if min_dist >= res.dist {
				 continue;
            }

            if max_dist < res.dist {
                stack_push(&stack, max_index);
            }
            stack_push(&stack, min_index);
        }
    }
    return res;
}
