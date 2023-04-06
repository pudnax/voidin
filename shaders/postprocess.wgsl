struct Globals {
    resolution: vec2<f32>,
    time: f32,
    frame: u32,
};

@group(0) @binding(0) var<uniform> un: Globals;
@group(1) @binding(0) var src_texture : texture_2d<f32>;
@group(1) @binding(1) var src_sampler : sampler;

struct VertexOutput {
  @builtin(position) pos: vec4<f32>,
  @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_idx: u32) -> VertexOutput {
    let uv = vec2<f32>(vec2((vertex_idx << 1u) & 2u, vertex_idx & 2u));
    let out = VertexOutput(vec4(2.0 * uv - 1.0, 0.0, 1.0), uv * vec2(1., -1.));
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
    let off = 0.005;
    let uv = uv + off / 2.;

    let shifted_col = vec3(
        textureSample(src_texture, src_sampler, uv + vec2(-off, off)).r,
        textureSample(src_texture, src_sampler, uv + vec2(-off, 0.)).g,
        textureSample(src_texture, src_sampler, uv + vec2(off, -off)).b,
    );

    return vec4(shifted_col, 1.);
}
