@group(0) @binding(0) var<uniform> camera: Camera;


struct LightInstance {
    @location(0) position: vec3<f32>,
    @location(1) radius: f32,
    @location(2) color: vec3<f32>,
}

struct VertexInput {
    @location(3) position: vec3<f32>,
}

@vertex
fn vs_main_stencil(
    instance: LightInstance,
    in: VertexInput,
) -> @builtin(position) vec4<f32> {
    let world_pos = in.position * instance.radius + instance.position;
    return camera.proj * camera.view * vec4<f32>(world_pos, 1.0);
}


struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) ndc: vec2<f32>,
    @location(1) @interpolate(linear) uv: vec2<f32>,

    @location(2) l_position: vec3<f32>,
    @location(3) l_inv_square_radius: f32,
    @location(4) l_color: vec3<f32>,
}

@vertex
fn vs_main_lighting(
    instance: LightInstance,
    in: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;

    let world_pos = in.position * instance.radius + instance.position;
    out.position = camera.proj * camera.view * vec4<f32>(world_pos, 1.0);
    out.ndc = out.position.xy / out.position.w;
    out.uv = out.ndc * vec2<f32>(0.5, -0.5) + 0.5;

    out.l_position = (camera.view * vec4<f32>(instance.position, 1.0)).xyz;
    out.l_inv_square_radius = 1.0 / (instance.radius * instance.radius);
    out.l_color = instance.color;

    return out;
}

@group(1) @binding(0) var t_albedo_metallic: texture_2d<f32>;
@group(1) @binding(1) var t_normal_roughness: texture_2d<f32>;
@group(1) @binding(2) var t_depth: texture_depth_2d;
@group(1) @binding(3) var t_sampler: sampler;

@fragment
fn fs_main_lighting(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = vec3(0.);
    return vec4<f32>(color, 1.0);
}
