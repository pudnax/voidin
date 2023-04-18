const PI = 3.141592;
const TAU = 6.283185;

fn hash11(x: f32) -> f32 {
    var p = fract(x * 0.1031);
    p *= p + 33.333;
    p *= p + p;
    return fract(p);
}

fn hash21(x: vec2<f32>) -> f32 {
    var p3 = fract(vec3(x.xyx) * .1031);
    p3 += dot(p3, p3.yzx + 33.333);
    return fract((p3.x + p3.y) * p3.z);
}

fn hash13(x: f32) -> vec3<f32> {
    var p3 = fract(vec3(x) * vec3(.1031, .1030, .0973));
    p3 = p3 + dot(p3, p3.yzx * 33.3333);
    return fract((p3.xxy + p3.yzz) * p3.zyx);
}

fn hash31(x: vec3<f32>) -> f32 {
    var p3 = fract(x * .1031);
    p3 += dot(p3, p3.zyx + 31.323);
    return fract((p3.x + p3.y) * p3.z);
}

fn hash33(x: vec3<f32>) -> vec3<f32> {
    var p3 = fract(x * vec3(.1031, .1030, .9073));
    p3 += dot(p3, p3.yxz + 31.323);
    return fract((p3.xxy + p3.yxx) * p3.zyx);
}

fn mat4_to_mat3(m: mat4x4<f32>) -> mat3x3<f32> {
    return mat3x3<f32>(m[0].xyz, m[1].xyz, m[2].xyz);
}

fn from_rotation_x(angle: f32) -> mat4x4<f32> {
    let s = sin(angle);
    let c = cos(angle);
    return mat4x4(
        vec4(1., 0., 0., 0.),
        vec4(0., c, s, 0.),
        vec4(0., -s, c, 0.),
        vec4(0., 0., 0., 1.),
    );
}

fn from_rotation_y(angle: f32) -> mat4x4<f32> {
    let s = sin(angle);
    let c = cos(angle);
    return mat4x4(
        vec4(c, 0., -s, 0.),
        vec4(0., 1., 0., 0.),
        vec4(s, 0., c, 0.),
        vec4(0., 0., 0., 1.),
    );
}

fn from_rotation_z(angle: f32) -> mat4x4<f32> {
    let s = sin(angle);
    let c = cos(angle);
    return mat4x4(
        vec4(c, s, 0., 0.),
        vec4(-s, c, 0., 0.),
        vec4(0., 0., 1., 0.),
        vec4(0., 0., 0., 1.),
    );
}
