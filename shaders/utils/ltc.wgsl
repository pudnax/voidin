// https://github.com/selfshadow/ltc_code
//
// Copyright (c) 2017, Eric Heitz, Jonathan Dupuy, Stephen Hill and David Neubelt.
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// * If you use (or adapt) the source code in your own work, please include a
//   reference to the paper:
//
//   Real-Time Polygonal-Light Shading with Linearly Transformed Cosines.
//   Eric Heitz, Jonathan Dupuy, Stephen Hill and David Neubelt.
//   ACM Transactions on Graphics (Proceedings of ACM SIGGRAPH 2016) 35(4), 2016.
//   Project page: https://eheitzresearch.wordpress.com/415-2/
//
// * Redistributions of source code must retain the above copyright notice, this
//   list of conditions and the following disclaimer.
//
// * Redistributions in binary form must reproduce the above copyright notice,
//   this list of conditions and the following disclaimer in the documentation
//   and/or other materials provided with the distribution.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
// FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
// DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
// SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
// CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
// OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.


const LTC1_TEXTURE = 2u;
const LTC2_TEXTURE = 3u;

const LUT_SIZE: f32 = 64.0;
const LUT_SCALE: f32 = 0.984375; // (LUT_SIZE - 1.0) / LUT_SIZE;
const LUT_BIAS: f32 = 0.0078125; // 0.5 / LUT_SIZE;

struct Ltc {
	matrix: mat3x3<f32>,
	t1: vec4<f32>,
	t2: vec4<f32>,
}

fn integrate_edge(v1: vec3<f32>, v2: vec3<f32>) -> vec3<f32> {
    let x = dot(v1, v2);
    let y = abs(x);

    let a = 0.8543985 + (0.4965155 + 0.0145206 * y) * y;
    let b = 3.4175940 + (4.1616724 + y) * y;
    let v = a / b;

    var theta_sintheta = v;
    if x <= 0.0 {
        theta_sintheta = 0.5 * inverseSqrt(max(1.0 - x * x, 1e-7)) - v;
    }

    return cross(v1, v2) * theta_sintheta;
}

fn sdsquare(p: vec2<f32>) -> f32 {
    var p = p - 0.5;
    p = abs(p) - 0.5;
    return length(max(p, vec2(0.))) + min(max(p.x, p.y), 0.);
}

fn gaussian_kernel(x: f32, sigma: f32) -> f32 {
    let s = 1. / sigma;
    return 0.39894 * exp(-0.5 * x * x * s * s) * s;
}

fn apply_texture(tex_idx: u32, p0: vec3<f32>, p1: vec3<f32>, p2: vec3<f32>) -> vec3<f32> {
    let v1 = p0 - p1;
    var v2 = p2 - p1;
    let plane_orto = cross(v1, v2);
    let plane_area_squared = dot(plane_orto, plane_orto);
    let dist_x_area = dot(plane_orto, p1);
    let p = dist_x_area * plane_orto / plane_area_squared - p1;

    let dot_v1_v2 = dot(v1, v2);
    let inv_dot_v1_v1 = 1. / dot(v1, v1);
    v2 = v2 - v1 * dot_v1_v2 * inv_dot_v1_v1;
    var uv: vec2<f32>;
    uv.y = dot(v2, p) / dot(v2, v2);
    uv.x = dot(v1, p) * inv_dot_v1_v1 - dot_v1_v2 * inv_dot_v1_v1 * uv.y;

    var sigma = abs(dist_x_area) / pow(plane_area_squared, 0.75);
    let add = max(0., sdsquare(uv));
    sigma += add;

    let y0 = gaussian_kernel(0., sigma);
    let y1 = y0 * 0.75;
    let x1 = gaussian_kernel(y1, sigma);
    let y2 = y0 * 0.5;
    let x2 = gaussian_kernel(y2, sigma);
    let y3 = y0 * 0.25;
    let x3 = gaussian_kernel(y3, sigma);

    let dx = vec2(0.5, 0.0);
    let dy = vec2(0.0, 0.5);

    var col = vec3(0.);
    col += textureSampleGrad(texture_array[tex_idx], tex_sampler, uv, dx * x3, dy * x3).rgb * 0.333;
    col += textureSampleGrad(texture_array[tex_idx], tex_sampler, uv, dx * x2, dy * x2).rgb * 0.333;
    col += textureSampleGrad(texture_array[tex_idx], tex_sampler, uv, dx * x1, dy * x1).rgb * 0.333;

    return col;
}

fn ltc_evaluate_rect(nor: vec3<f32>, view: vec3<f32>, pos: vec3<f32>, minv: mat3x3<f32>, points: array<vec3<f32>,4>, two_sided: bool) -> vec3<f32> {
    let T1 = normalize(view - nor * dot(view, nor));
    let T2 = cross(nor, T1);

    let minv = minv * transpose(mat3x3(T1, T2, nor));

    var L = array<vec3<f32>, 4>(
        minv * (points[0] - pos),
        minv * (points[1] - pos),
        minv * (points[2] - pos),
        minv * (points[3] - pos),
    );

    let dir = points[0] - pos;
    let light_normal = cross(points[1] - points[0], points[3] - points[0]);
    let behind = dot(dir, light_normal) < 0.0;

    L[0] = normalize(L[0]);
    L[1] = normalize(L[1]);
    L[2] = normalize(L[2]);
    L[3] = normalize(L[3]);

    var vsum = vec3(0.0);
    vsum += integrate_edge(L[0], L[1]);
    vsum += integrate_edge(L[1], L[2]);
    vsum += integrate_edge(L[2], L[3]);
    vsum += integrate_edge(L[3], L[0]);

    let len = length(vsum);

    var z = vsum.z / len;
    if behind {
        z = -z;
    }

    var uv = vec2(z * 0.5 + 0.5, len);
    uv = uv * LUT_SCALE + LUT_BIAS;

    let scale = textureSample(texture_array[LTC2_TEXTURE], tex_ltc_sampler, uv).w;

    var sum = len * scale;
    if behind && !two_sided {
        sum = 0.0;
    }

    return vec3(sum);
}

fn ltc_matrix(nor: vec3<f32>, view: vec3<f32>, roughness: f32) -> Ltc {
    let ndotv = saturate(dot(nor, view));
    var uv = vec2(roughness, sqrt(1.0 - ndotv));
    uv = uv * LUT_SCALE + LUT_BIAS;

    let t1 = textureSample(texture_array[LTC1_TEXTURE], tex_sampler, uv);
    let t2 = textureSample(texture_array[LTC2_TEXTURE], tex_sampler, uv);

    var res: Ltc;
    res.t1 = t1;
    res.t2 = t2;
    res.matrix = mat3x3(
        vec3(t1.x, 0., t1.y),
        vec3(0., 1., 0.),
        vec3(t1.z, 0., t1.w),
    );

    return res;
}

fn get_area_light_diffuse(nor: vec3<f32>, view: vec3<f32>, pos: vec3<f32>, points: array<vec3<f32>,4>, two_sided: bool) -> vec3<f32> {
    let one = mat3x3(vec3(1., 0., 0.), vec3(0., 1., 0.), vec3(0., 0., 1.));
    return ltc_evaluate_rect(nor, view, pos, one, points, two_sided);
}

// FIXME: pass `Ltc` as a pointer
fn get_area_light_specular(nor: vec3<f32>, view: vec3<f32>, pos: vec3<f32>, ltc: Ltc, points: array<vec3<f32>,4>, two_sided: bool, scolor: vec3<f32>) -> vec3<f32> {
    var spec = ltc_evaluate_rect(nor, view, pos, ltc.matrix, points, two_sided);
    spec *= scolor * ltc.t2.x + (1.0 - scolor) * ltc.t2.y;
    return spec;
}
