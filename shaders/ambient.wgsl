#import <shared.wgsl>

@group(0) @binding(0) var t_positions: texture_2d<f32>;
@group(0) @binding(1) var t_normal: texture_2d<f32>;
@group(0) @binding(2) var t_material: texture_2d<u32>;
@group(0) @binding(3) var t_depth: texture_depth_2d;
@group(0) @binding(4) var t_sampler: sampler;
@group(0) @binding(5) var t_int_sampler: sampler;

@group(1) @binding(0) var texture_array: binding_array<texture_2d<f32>>;
@group(1) @binding(1) var tex_sampler: sampler;

@group(2) @binding(0) var<storage, read> materials: array<Material>;

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

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let positions_tex = textureSample(t_positions, t_sampler, in.uv);
    let normal_tex = textureSample(t_normal, t_sampler, in.uv);
    let material_id = textureGather(t_material, t_int_sampler, in.uv).r;
    let material = materials[material_id];
    let uv = vec2(positions_tex.w, normal_tex.w) * vec2(1., 1.);

    var albedo = textureSample(texture_array[material.albedo], t_sampler, uv);
    var emissive = textureSample(texture_array[material.emissive], t_sampler, uv).rgb;
    return vec4(albedo.rgb + emissive, 1.0);
}
