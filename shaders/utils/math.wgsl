const PI = 3.141592;
const TAU = 6.283185;
const EPS: f32 = 0.0001;
const MAX_DIST: f32 = 1e30;

fn min_element(x: vec3<f32>) -> f32 {
    return min(x.x, min(x.y, x.z));
}

fn max_element(x: vec3<f32>) -> f32 {
    return max(x.x, max(x.y, x.z));
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

fn extract_translation(mat: mat4x4<f32>) -> vec3<f32> {
    return mat[3].xyz;
}

fn extract_scale(mat: mat4x4<f32>) -> vec3<f32> {
    return vec3(
        length(mat[0].xyz),
        length(mat[1].xyz),
        length(mat[2].xyz),
    );
}

fn extract_rotation(mat: mat4x4<f32>) -> mat4x4<f32> {
    let scale = extract_scale(mat);
    return mat4x4(
        vec4(vec3(mat[0].xyz / scale), 0.),
        vec4(vec3(mat[1].xyz / scale), 0.),
        vec4(vec3(mat[2].xyz / scale), 0.),
        vec4(0., 0., 0., 1.),
    );
}
