struct VertexOutput {
  @builtin(position) pos: vec4<f32>,
  @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main_trig(@builtin(vertex_index) vertex_idx: u32) -> VertexOutput {
    let x = (1. - f32(vertex_idx)) * 0.5;
    let y = f32(vertex_idx & 1u) - 0.5;
    let clip_pos = vec4(x, y, 0., 1.);
    return VertexOutput(clip_pos, clip_pos.xy);
}

@vertex
fn vs_main_full(@builtin(vertex_index) vertex_idx: u32) -> VertexOutput {
    let uv = vec2<f32>(vec2((vertex_idx << 1u) & 2u, vertex_idx & 2u));
    let out = VertexOutput(vec4(2.0 * uv.x - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0), uv);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4(vec3(in.uv, 0.3), 1.);
}
