#import <shared.wgsl>
#import <utils.wgsl>

@group(0) @binding(0) var<uniform> un: Globals;
@group(1) @binding(0)
var<storage, read> indices: array<u32>;
@group(2) @binding(0)
var<storage, read_write> instances: array<Instance>;

@compute
@workgroup_size(64, 1, 1)
fn update(@builtin(global_invocation_id) global_id: vec3<u32>) {
    if global_id.x >= arrayLength(&indices) {
        return;
    }
    var idx = indices[global_id.x];
    let instance = &instances[idx - 0u];
    let transform = (*instance).transform;

    var speed = 1.0;
    if transform[3][2] > 0.01 {
        speed *= 1.0;
    } else {
        speed *= -1.0;
    }
    let rot = from_rotation_z(speed * un.dt);
    (*instance).transform = rot * transform;
}
