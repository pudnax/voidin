#import <shared.wgsl>
#import <utils/uv.wgsl>

@group(0) @binding(0) var<uniform> camera: Camera;

@group(1) @binding(0) var t_normal_uv: texture_2d<u32>;
@group(1) @binding(1) var t_material: texture_2d<u32>;
@group(1) @binding(2) var t_depth: texture_depth_2d;
@group(1) @binding(3) var t_sampler: sampler;

struct VertexOutput {
  @builtin(position) pos: vec4<f32>,
  @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_idx: u32) -> VertexOutput {
    var out: VertexOutput;
    out.uv = vec2<f32>(vec2((vertex_idx << 1u) & 2u, vertex_idx & 2u));
    out.pos = vec4(2.0 * out.uv.x - 1.0, 1. - out.uv.y * 2., 0.0, 1.0);
    return out;
}

@fragment
fn fs_main(
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
) -> @location(0) vec4<f32> {
    let tex_dims = vec2f(textureDimensions(t_depth));
    let pix = vec2<i32>(uv * tex_dims);

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

    let inv_dims = 1.0 / tex_dims;
    let limits = all(prev_position_ndc.xy == clamp(prev_position_ndc.xy, -1. + inv_dims, 1. - inv_dims));
    return vec4(velocity, f32(limits), 1.);
}
