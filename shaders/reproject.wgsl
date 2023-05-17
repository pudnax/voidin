#import <shared.wgsl>
#import <utils/uv.wgsl>

@group(0) @binding(0) var<uniform> camera: Camera;

@group(1) @binding(0) var t_normal_uv: texture_2d<u32>;
@group(1) @binding(1) var t_material: texture_2d<u32>;
@group(1) @binding(2) var t_depth: texture_depth_2d;
@group(1) @binding(3) var t_sampler: sampler;

@group(2) @binding(0) var t_motion: texture_storage_2d<rgba16float, write>;

@compute
@workgroup_size(8, 8, 1)
fn cs_main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let pix = vec2<i32>(global_id.xy);
    let dims = textureDimensions(t_motion);
    let uv = get_uv_comp(global_id, dims);

    var depth = 0.0;
    for (var y = -1; y <= 1; y += 1) {
        for (var x = -1; x <= 1; x += 1) {
            let d = textureLoad(t_depth, pix + vec2(x, y), 0);
            depth = max(depth, d);
        }
    }

    let curr_position_ndc = vec4(ndc_from_uv_raw_depth(uv, depth), 1.);

    let pos_ws = world_position_from_depth(uv, depth, camera.inv_proj_view);
    let prev_position_ndc_w = camera.prev_world_to_clip * vec4(pos_ws, 1.);
    let prev_position_ndc = prev_position_ndc_w.xyz / prev_position_ndc_w.w;

    let velocity = (curr_position_ndc.xy + camera.jitter) - (prev_position_ndc.xy + camera.prev_jitter);

    let inv_dims = 1.0 / vec2<f32>(dims);
    let limits = all(prev_position_ndc.xy == clamp(prev_position_ndc.xy, -1. + inv_dims, 1. - inv_dims));
    textureStore(t_motion, pix, vec4(velocity, f32(limits), 1.));
}
