struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_idx: u32) -> VertexOutput {
    let uv = vec2(f32(vertex_idx & 2u), f32((vertex_idx << 1u) & 2u));
    let pos = vec4(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0);
    return VertexOutput(pos, uv);
}

@group(0) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(0) var tex_sampler: sampler;

fn srgb_to_linear(rgb: vec3<f32>) -> vec3<f32> {
    return select(
        pow((rgb + 0.055) * (1.0 / 1.055), vec3<f32>(2.4)),
        rgb * (1.0 / 12.92),
        rgb <= vec3<f32>(0.04045)
    );
}

fn linear_to_srgb(rgb: vec3<f32>) -> vec3<f32> {
    return select(
        1.055 * pow(rgb, vec3(1.0 / 2.4)) - 0.055,
        rgb * 12.92,
        rgb <= vec3<f32>(0.0031308)
    );
}

@fragment
fn fs_main_srgb(vout: VertexOutput) -> @location(0) vec4<f32> {
    let tex = textureSample(tex, tex_sampler, vout.tex_coords);
    return vec4(linear_to_srgb(tex.rgb), tex.a);
}

@fragment
fn fs_main(vout: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(tex, tex_sampler, vout.tex_coords);
}
