const LIGHT_MATERIAL = 1u;
const WHITE_TEXTURE = 0u;
const BLACK_TEXTURE = 1u;

struct Globals {
    resolution: vec2<f32>,
    frame: u32,
    time: f32,
	dt: f32,
	custom: f32,
}

struct Camera {
	position: vec4<f32>,
	proj: mat4x4<f32>,
	view: mat4x4<f32>,
	clip_to_world: mat4x4<f32>,
	prev_world_to_clip: mat4x4<f32>,
	frustum: vec4<f32>,
	zfar: f32, znear: f32,
	jitter: vec2<f32>,
	prev_jitter: vec2<f32>,
	padding: vec2<f32>,
}

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
	min: vec3<f32>,
	index_count: u32,
	max: vec3<f32>,
	base_index: u32,
    vertex_offset: i32,
	bvh_index: u32,
	junk: vec2<f32>,
}

struct Instance {
    transform: mat4x4<f32>,
    inv_transform: mat4x4<f32>,
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
