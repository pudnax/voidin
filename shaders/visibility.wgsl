#import <shared.wgsl>
#import <utils.wgsl>
#import <encoding.wgsl>

@group(0) @binding(0) var<uniform> camera: Camera;
@group(1) @binding(0) var texture_array: binding_array<texture_2d<f32>>;
@group(1) @binding(1) var tex_sampler: sampler;
@group(1) @binding(2) var tex_int_sampler: sampler;

// FIXME: add more bind groups for only read storage
@group(2) @binding(0) var<storage, read_write> instances: array<Instance>;
@group(3) @binding(0) var<storage, read> materials: array<Material>;

struct VertexInput {
	@builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec4<f32>,
    @location(3) tex_coords: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec3<f32>,
    @location(3) bitangent: vec3<f32>,
    @location(4) uv: vec2<f32>,
    @location(5) @interpolate(flat) material_id: u32,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    let instance = instances[in.instance_index];

    let world_pos = instance.transform * vec4(in.position, 1.0);
    let view_pos = camera.view * world_pos;

    var out: VertexOutput;

    out.clip_position = camera.proj * view_pos;

    var transform = mat4_to_mat3(instance.transform);
    out.normal = transform * in.normal;
    out.tangent = transform * in.tangent.xyz;
    out.bitangent = cross(out.normal, out.tangent) * in.tangent.w;

    out.uv = in.tex_coords;
    out.material_id = instance.material_id;

    return out;
}

struct FragmentOutput {
    @location(0) normal_uv: vec2<u32>,
    @location(1) @interpolate(flat) material: u32,
}

fn get_tbn(normal: vec3<f32>, tangent: vec3<f32>, bitangent: vec3<f32>) -> mat3x3<f32> {
    return mat3x3(
        normalize(tangent),
        normalize(bitangent),
        normalize(normal)
    );
}

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    let uv = in.uv;
    let material = materials[in.material_id];
    let albedo_tex = textureSample(texture_array[material.albedo], tex_sampler, uv);
    let normal_tex = textureSample(texture_array[material.normal], tex_sampler, uv);

    if material.base_color.w < 0.5 || albedo_tex.a < 0.5 {
     	 discard;
    }

    var normal = vec3(0.);
    if material.normal == 0u {
        normal = normalize(in.normal);
    } else {
        let tbn = get_tbn(in.normal, in.tangent, in.bitangent);
        normal = normalize(tbn * (normal_tex.rgb * 2.0 - 1.0));
    }

    let packed_norm = encode_octahedral_32(normal);

    return FragmentOutput(
        vec2(packed_norm, pack2x16float(in.uv)),
        in.material_id
    );
}
