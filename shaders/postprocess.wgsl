#import "shared.wgsl"
#import "utils/color.wgsl"

@group(0) @binding(0) var<uniform> un: Globals;
@group(1) @binding(0) var src_texture : texture_2d<f32>;
@group(2) @binding(0) var src_sampler : sampler;

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

fn tonemap_curve(v: f32) -> f32 {
    let c = v + v * v + 0.5 * v * v * v;
    return c / (1.0 + c);
}

fn tonemap_curve_vec(col: vec3<f32>) -> vec3<f32> {
    return vec3(tonemap_curve(col.r), tonemap_curve(col.g), tonemap_curve(col.b));
}

fn neutral_tonemap(col: vec3<f32>) -> vec3<f32> {
    let ycbcr = rgb_to_ycbcr(col);

    let chroma = length(ycbcr.yz) * 2.4;
    let bt = tonemap_curve(chroma);

    var desat = max((bt - 0.7) * 0.8, 0.0);
    desat *= desat;

    let desat_col = mix(col, ycbcr.xxx, desat);

    let tm_luma = tonemap_curve(ycbcr.x);
    let tm0 = col * max(0.0, tm_luma / max(1e-5, calculate_luma(col)));
    let final_mult = 0.97;
    let tm1 = tonemap_curve_vec(desat_col);

    let res = mix(tm0, tm1, vec3(bt * bt));
    return res * final_mult;
}

fn sharpen_remap(l: f32) -> f32 {
    return sqrt(l);
}

fn sharpen_remap_inv(l: f32) -> f32 {
    return l * l;
}

@fragment
fn fs_main(
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
) -> @location(0) vec4<f32> {
    var col = textureSample(src_texture, src_sampler, uv).rgb;
    let dims_inv = 1. / vec2<f32>(textureDimensions(src_texture));

    let sharpen_amount = 0.5;

    var neighbours = 0.;
    var wt_sum = 0.;

    let dim_offsets = array<vec2<f32>, 2>(vec2(1., 0.), vec2(0., 1.));

    let center = sharpen_remap(calculate_luma(col));
    var wts: vec2f;

    for (var dim = 0; dim < 2; dim += 1) {
        let n0coord = uv + dim_offsets[0] * dims_inv;
        let n1coord = uv + dim_offsets[1] * dims_inv;

        let n0 = sharpen_remap(calculate_luma(textureSampleLevel(src_texture, src_sampler, n0coord, 0.).rgb));
        let n1 = sharpen_remap(calculate_luma(textureSampleLevel(src_texture, src_sampler, n1coord, 0.).rgb));
        var wt = max(0., 1. - 6. * (abs(center - n0) + abs(center - n1)));
        wt = min(wt, sharpen_amount * wt * 1.25);

        neighbours += n0 * wt;
        neighbours += n1 * wt;
        wt_sum += wt * 2.;
    }

    var sharpened_luma = max(0., center * (wt_sum + 1.) - neighbours);
    sharpened_luma = sharpen_remap_inv(sharpened_luma);

    col *= max(0.0, sharpened_luma / max(1e-5, calculate_luma(col.rgb)));

    col = neutral_tonemap(col);

    return vec4(col, 1.);
}
