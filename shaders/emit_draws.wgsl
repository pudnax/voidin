#import <shared.wgsl>
#import <utils/math.wgsl>

@group(0) @binding(0)
var<uniform> camera: Camera;
@group(1) @binding(0)
var<storage, read> meshes: array<MeshInfo>;
@group(2) @binding(0)
var<storage, read_write> instances: array<Instance>;
@group(3) @binding(0)
var<storage, read_write> cmd_buffer: array<DrawIndexedIndirect>;

fn is_visible(mesh: MeshInfo, transform: mat4x4<f32>, scale: vec3<f32>) -> bool {
    var center = (mesh.max + mesh.min) / 2.;
    center = (camera.view * transform * vec4(center, 1.0)).xyz;

    let abs_scale = abs(scale);
    let max_scale = max(max(abs_scale.x, abs_scale.y), abs_scale.z);
    let radius = max(distance(mesh.min, center), distance(mesh.max, center)) * max_scale;

    if center.z * camera.frustum.y - abs(center.x) * camera.frustum.x < -radius {
        return false;
    }
    if center.z * camera.frustum.w - abs(center.y) * camera.frustum.z < -radius {
        return false;
    }

    if center.z + radius > camera.znear && center.z - radius > camera.zfar {
        return false;
    }

    return true;
}

@compute
@workgroup_size(64, 1, 1)
fn emit_draws(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    let len = arrayLength(&instances);
    if index >= len {
        return;
    }

    let instance = instances[global_id.x];
    let transform = instance.transform;
    let mesh_info = meshes[instance.mesh_id];

    let scale = extract_scale(transform);

    var instance_count = 1u;
    if !is_visible(mesh_info, transform, scale) {
        instance_count = 0u;
    }

    var cmd: DrawIndexedIndirect;

    cmd.vertex_count = mesh_info.index_count;
    cmd.instance_count = instance_count;
    cmd.base_index = mesh_info.base_index;
    cmd.vertex_offset = mesh_info.vertex_offset;
    cmd.base_instance = global_id.x;

    cmd_buffer[global_id.x] = cmd;
}
