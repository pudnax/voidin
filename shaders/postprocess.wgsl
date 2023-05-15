#import <shared.wgsl>

@group(0) @binding(0) var<uniform> un: Globals;
@group(1) @binding(0) var src_texture : texture_2d<f32>;
@group(1) @binding(1) var src_sampler : sampler;

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

@vertex
fn vs_main_trig(@builtin(vertex_index) vertex_idx: u32) -> VertexOutput {
    let x = (1. - f32(vertex_idx)) * 0.5;
    let y = f32(vertex_idx & 1u) - 0.5;
    let clip_pos = vec4(x, y, 0., 1.);
    return VertexOutput(clip_pos, clip_pos.xy);
}

@fragment
fn fs_main(
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
) -> @location(0) vec4<f32> {
    let off = 0.0015;
    let uv = uv + off / 2.;

    let shifted_col = vec3(
        textureSample(src_texture, src_sampler, uv + vec2(-off, off)).r,
        textureSample(src_texture, src_sampler, uv + vec2(-off, 0.)).g,
        textureSample(src_texture, src_sampler, uv + vec2(off, -off)).b,
    );

    return vec4(shifted_col, 1.);
}
