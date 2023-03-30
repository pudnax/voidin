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

@group(0) @binding(0) var<uniform> un: Globals;
@group(1) @binding(0) var<uniform> camera: Camera;
@group(2) @binding(0) var<uniform> model: mat4x4<f32>;

struct Material {
    base_color_factor: vec3<f32>,
    alpha_cutoff: f32,
};
@group(3) @binding(0) var<uniform> material : Material;
@group(3) @binding(1) var base_color_texture : texture_2d<f32>;
@group(3) @binding(2) var material_sampler : sampler;

struct VertexInput {
	@location(0) pos: vec3<f32>,
	@location(1) normal: vec3<f32>,
	@location(2) tex_coords: vec2<f32>,
}

struct VertexOutput {
	@builtin(position) pos: vec4<f32>,
	@location(0) normal: vec3<f32>,
	@location(1) tex_coords: vec2<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    let pos = camera.proj * camera.view * model * vec4(in.pos, 1.0);
    let normal = normalize((camera.view * model * vec4(in.normal, 0.0)).xyz);
    let tex_coords = in.tex_coords;

    return VertexOutput(pos, normal, tex_coords);
}

const LIGHT_DIR = vec3<f32>(0.25, 0.5, 1.0);
const LIGHT_COLOR = vec3<f32>(1.0, 1.0, 1.0);
const AMBIENT_COLOR = vec3<f32>(0.1, 0.1, 0.1);

@fragment
fn fs_main_cutoff(vout: VertexOutput) -> @location(0) vec4<f32> {
    let material_texture = textureSample(base_color_texture, material_sampler, vout.tex_coords);

    if material_texture.a < material.alpha_cutoff {
        discard;
    }

    let nor = normalize(vout.normal);
    let light_dir = normalize(LIGHT_DIR);
    let diff = max(dot(nor, light_dir), 0.0);
    let base_color = material_texture.rgb * material.base_color_factor;
    let surface_color = (base_color * AMBIENT_COLOR) + (base_color * diff);

    return vec4(surface_color, material_texture.a);
}

@fragment
fn fs_main(vout: VertexOutput) -> @location(0) vec4<f32> {
    let material_texture = textureSample(base_color_texture, material_sampler, vout.tex_coords);

    let nor = normalize(vout.normal);
    let light_dir = normalize(LIGHT_DIR);
    let diff = max(dot(nor, light_dir), 0.0);
    let base_color = material_texture.rgb * material.base_color_factor;
    let surface_color = (base_color * AMBIENT_COLOR) + (base_color * diff);

    return vec4(surface_color, material_texture.a);
}
