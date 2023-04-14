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

fn mat4_to_mat3(m: mat4x4<f32>) -> mat3x3<f32> {
    return mat3x3<f32>(m[0].xyz, m[1].xyz, m[2].xyz);
}
