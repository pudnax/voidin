#import <shared.wgsl>
#import <utils.wgsl>

@group(0) @binding(0) var<uniform> un: Globals;
@group(1) @binding(0) var<uniform> camera: Camera;
@group(2) @binding(0) var texture_array: binding_array<texture_2d<f32>>;
@group(2) @binding(1) var tex_sampler: sampler;

@group(3) @binding(0) var<storage, read> meshes: array<MeshInfo>;
// FIXME: add more bind groups for only read storage
@group(4) @binding(0) var<storage, read_write> instances: array<Instance>;
@group(5) @binding(0) var<storage, read> materials: array<Material>;

struct VertexInput {
	@builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) @interpolate(flat) material_id: u32,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    let instance = instances[in.instance_index];

    let world_pos = instance.transform * vec4(in.position, 1.0);
    let view_pos = camera.view * world_pos;

    var out: VertexOutput;

    out.clip_position = camera.proj * view_pos;
    out.position = view_pos.xyz / view_pos.w;
    out.normal = mat4_to_mat3(instance.transform) * in.normal;
    out.uv = in.tex_coords;
    out.material_id = instance.material_id;

    return out;
}

struct FragmentOutput {
    @location(0) albedo_metallic: vec4<f32>,
    @location(1) normal: vec4<f32>,
    @location(2) emissive_rough: vec4<f32>,
}

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    let material = materials[in.material_id];
    let albedo_tex = textureSample(texture_array[material.albedo], tex_sampler, in.uv);
    let normal_tex = textureSample(texture_array[material.normal], tex_sampler, in.uv);
    let emissive_tex = textureSample(texture_array[material.emissive], tex_sampler, in.uv);
    let metal_rough_tex = textureSample(texture_array[material.metallic_roughness], tex_sampler, in.uv).bg;

    if material.base_color.w < 0.5 || albedo_tex.a < 0.5 {
     	discard;
    }

    return FragmentOutput(
        vec4(albedo_tex.rgb, metal_rough_tex.x),
        vec4(normal_tex.rgb, 1.0),
        vec4(emissive_tex.rgb, metal_rough_tex.y),
    );
}