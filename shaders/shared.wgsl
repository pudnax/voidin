struct Globals {
    resolution: vec2<f32>,
    frame: u32,
    time: f32,
	dt: f32,
	custom: f32,
	prev_jitter: vec2<f32>,
	jitter: vec2<f32>,
};

struct Camera {
	position: vec4<f32>,
	proj: mat4x4<f32>,
	view: mat4x4<f32>,
	inv_proj_view: mat4x4<f32>,
	frustum: vec4<f32>,
	zfar: f32, znear: f32,
};

struct Light {
	position: vec3<f32>,
	radius: f32,
	color: vec3<f32>
}

struct AreaLight {
	color: vec3<f32>,
	intensity: f32,
	points: array<vec3<f32>, 4>,
}

struct BoundingSphere {
	center: vec3<f32>,
	radius: f32,
}

struct MeshInfo {
	index_count: u32,
	base_index: u32,
    vertex_offset: i32,
	padding: f32,
	bounding_sphere: BoundingSphere,
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
