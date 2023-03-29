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

struct VertexInput {
	@location(0) pos: vec3<f32>,
	@location(1) normal: vec3<f32>,
	@location(2) tex_coords: vec2<f32>,
}

struct VertexOutput {
	@builtin(position) pos: vec4<f32>,
	@location(0) normal: vec3<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    let pos = camera.proj * camera.view * model * vec4(in.pos, 1.0);
    let normal = normalize((camera.view * model * vec4(in.normal, 0.0)).xyz);

    return VertexOutput(pos, normal);
}

const lightDir = vec3<f32>(0.25, 0.5, 1.0);
const lightColor = vec3<f32>(1.0, 1.0, 1.0);
const ambientColor = vec3<f32>(0.1, 0.1, 0.1);

@fragment
fn fs_main(vout: VertexOutput) -> @location(0) vec4<f32> {
    let N = normalize(vout.normal);
    let L = normalize(lightDir);
    let NDotL = max(dot(N, L), 0.0);
    let surfaceColor = ambientColor + NDotL;

    return vec4(surfaceColor, 1.0);
}
