#import <shared.wgsl>

@group(0) @binding(0) var<uniform> camera: Camera;

@group(1) @binding(0) var t_normal_uv: texture_2d<u32>;
@group(1) @binding(1) var t_material: texture_2d<u32>;
@group(1) @binding(2) var t_depth: texture_depth_2d;
@group(1) @binding(3) var t_sampler: sampler;

@group(2) @binding(0) var motion_texture: texture_storage_2d<rgba16float, write>;


@compute
@workgroup_size(8, 8, 1)
fn cs_main(@builtin(global_invocation_id) global_id: vec3<u32>) {
}
