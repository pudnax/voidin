#include "shared.wgsl";

@group(0) @binding(0)
var<storage, read> meshes: array<MeshInfo>;

@group(1) @binding(0)
var<storage, read> instances: array<Instance>;

@group(2) @binding(0)
var<storage, read_write> cmd_buffer: array<DrawIndexedIndirect>;

@compute
@workgroup_size(32, 1, 1)
fn emit_draws(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    let len = arrayLength(&instances);
    if index >= len {
        return;
    }

    let mesh_id = instances[global_id.x].mesh_id;
    let mesh_info = meshes[mesh_id];

    var cmd: DrawIndexedIndirect;

    cmd.vertex_count = mesh_info.index_count;
    cmd.instance_count = 1u;
    cmd.base_index = mesh_info.base_index;
    cmd.vertex_offset = mesh_info.vertex_offset;
    cmd.base_instance = global_id.x;

    cmd_buffer[global_id.x] = cmd;
}
