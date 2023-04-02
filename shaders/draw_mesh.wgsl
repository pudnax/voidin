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
	@location(2) light_vec: vec3<f32>,
	@location(3) view_vec: vec3<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    let LIGHT_POS = vec3(15., 10.5, 15.);

    let vpos = camera.proj * camera.view * model * vec4(in.pos, 1.0);
    let pos = camera.view * model * vec4(in.pos, 1.0);
    let normal = normalize((model * vec4(in.normal, 0.0)).xyz);
    let tex_coords = in.tex_coords;
    var light_vec = LIGHT_POS - pos.xyz;
    // light_vec = (model * vec4(light_vec, 1.0)).rgb;
    var view_vec = camera.position - pos.xyz;
    view_vec = (model * vec4(view_vec, 1.0)).rgb;

    return VertexOutput(vpos, normal, tex_coords, light_vec, view_vec);
}

fn shade(nor: vec3<f32>, light_dir: vec3<f32>, view: vec3<f32>, material_texture: vec4<f32>) -> vec3<f32> {
    let refl = reflect(light_dir, nor);
    let spec = pow(max(dot(refl, view), 0.0), 16.0) * vec3(0.015);

    let shade = dot(nor, light_dir);
    let diff = mix(max(shade, 0.0), shade * 0.5 + 0.5, 0.25) * material.base_color_factor;
    var surface_color = material_texture.rgb * diff + spec;

    return surface_color;
}

@fragment
fn fs_main_cutoff(vout: VertexOutput) -> @location(0) vec4<f32> {
    let material_texture = textureSample(base_color_texture, material_sampler, vout.tex_coords);

    if material_texture.a < material.alpha_cutoff {
        discard;
    }

    let nor = normalize(vout.normal);
    let light_dir = normalize(vout.light_vec);
    let view = normalize(vout.view_vec);

    let surface_color = shade(nor, light_dir, view, material_texture);

    return vec4(surface_color, material_texture.a);
}

@fragment
fn fs_main(vout: VertexOutput) -> @location(0) vec4<f32> {
    let material_texture = textureSample(base_color_texture, material_sampler, vout.tex_coords);

    let nor = normalize(vout.normal);
    let light_dir = normalize(vout.light_vec);
    let view = normalize(vout.view_vec);

    let surface_color = shade(nor, light_dir, view, material_texture);

    return vec4(surface_color, material_texture.a);
}
