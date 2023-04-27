const LTC1_TEXTURE = 2;
const LTC2_TEXTURE = 3;

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

fn ltc_evaluate(nor: vec3<f32>, view: vec3<f32>, pos: vec3<f32>, minv: mat3x3<f32>, points: array<vec3<f32>,4>, two_sided: bool) -> vec3<f32> {
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

    let scale = textureSample(texture_array[LTC1_TEXTURE], tex_ltc_sampler, uv).w;

    var sum = len * scale;
    if !behind && two_sided {
        sum = 0.0;
    }

    return vec3(sum);
}

fn ltc_matrix(nor: vec3<f32>, view: vec3<f32>, roughness: f32) -> Ltc {
    let ndotv = clamp(dot(nor, view), 0., 1.);
    var uv = vec2(roughness, sqrt(1.0 - ndotv));
    uv = uv * LUT_SCALE + LUT_BIAS;

    let t1 = textureSample(texture_array[LTC1_TEXTURE], tex_ltc_sampler, uv);
    let t2 = textureSample(texture_array[LTC2_TEXTURE], tex_ltc_sampler, uv);

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
    let one = mat3x3(1., 1., 1., 1., 1., 1., 1., 1., 1.);
    return ltc_evaluate(nor, view, pos, one, points, two_sided);
}

// FIXME: pass `Ltc` as a pointer
fn get_area_light_specular(nor: vec3<f32>, view: vec3<f32>, pos: vec3<f32>, ltc: Ltc, points: array<vec3<f32>,4>, two_sided: bool, scolor: vec3<f32>) -> vec3<f32> {
    var spec = ltc_evaluate(nor, view, pos, ltc.matrix, points, two_sided);
    spec *= scolor * ltc.t2.x + (1.0 - scolor) * ltc.t2.y;
    return spec;
}
