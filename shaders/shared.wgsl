#pragma once

struct Globals {
    resolution: vec2<f32>,
    time: f32,
    frame: u32,
};

struct Camera {
	position: vec3<f32>,
	proj: mat4x4<f32>,
	view: mat4x4<f32>,
	inv_proj: mat4x4<f32>,
};

struct MeshInfo {
    vertex_offset: u32,
	vertex_count: u32,
	base_index: u32,
	index_count: u32,
}

struct Instance {
    transform: mat4x4<f32>,
	mesh_id: u32,
	material_id: u32,
	padding: vec2<f32>,
}

struct Material {
    base_color: vec4<f32>,
	albedo: u32,
	normal: u32,
	metallic_roughness: u32,
	emissive: u32,
}

struct DrawIndexedIndirect {
    vertex_count: u32,
    instance_count: u32,
    base_index: u32,
    vertex_offset: i32,
    base_instance: u32,
}

const PI = 3.141592;
const TAU = 6.283185;

fn hash13(x: f32) -> vec3<f32> {
    var p3 = fract(vec3(x) * vec3(.1031, .1030, .0973));
    p3 = p3 + dot(p3, p3.yzx * 33.3333);
    return fract((p3.xxy + p3.yzz) * p3.zyx);
}

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
