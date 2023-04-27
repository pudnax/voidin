#import <shared.wgsl>
#import <ltc_utils.wgsl>

@group(0) @binding(0) var<uniform> global: Globals;

@group(1) @binding(0) var<uniform> camera: Camera;

@group(2) @binding(0) var t_positions: texture_2d<f32>;
@group(2) @binding(1) var t_normal: texture_2d<f32>;
@group(2) @binding(2) var t_material: texture_2d<u32>;
@group(2) @binding(3) var t_depth: texture_depth_2d;
@group(2) @binding(4) var t_sampler: sampler;
@group(2) @binding(5) var t_int_sampler: sampler;

@group(3) @binding(0) var texture_array: binding_array<texture_2d<f32>>;
@group(3) @binding(1) var tex_sampler: sampler;
@group(3) @binding(2) var tex_ltc_sampler: sampler;

@group(4) @binding(0) var<storage, read> materials: array<Material>;

@group(5) @binding(0) var<storage, read> point_lights: array<Light>;
@group(6) @binding(0) var<storage, read> area_lights: array<AreaLight>;

struct VertexOutput {
  @builtin(position) pos: vec4<f32>,
  @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_idx: u32) -> VertexOutput {
    let uv = vec2<f32>(vec2((vertex_idx << 1u) & 2u, vertex_idx & 2u));
    let out = VertexOutput(vec4(2.0 * uv - 1.0, 0.0, 1.0), uv * vec2(1., -1.));
    return out;
}

fn sqr(x: f32) -> f32 {
    return x * x;
}

fn attenuation(max_intensity: f32, falloff: f32, dist: f32, radius: f32) -> f32 {
    var s = dist / radius;
    if s >= 1.0 {
        return 0.;
    }
    let s2 = sqr(s);
    return max_intensity * sqr(1. - s2) / (1. + falloff * s2);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let positions_tex = textureSample(t_positions, t_sampler, in.uv);
    let normal_tex = textureSample(t_normal, t_sampler, in.uv);
    let material_id = textureGather(t_material, t_int_sampler, in.uv).r;
    let uv = vec2(positions_tex.w, normal_tex.w) * vec2(1., 1.);

    let material = materials[material_id];
    let albedo = textureSample(texture_array[material.albedo], t_sampler, uv);
    let emissive = textureSample(texture_array[material.emissive], t_sampler, uv).rgb;
    let metallic_roughness = textureSample(texture_array[material.metallic_roughness], t_sampler, uv);

    let pos = positions_tex.xyz;
    let nor = normal_tex.rgb;
    let rd = -normalize(camera.position.xyz - pos);

    var color = vec3(0.);

    color = albedo.rgb * 0.05 + emissive;

    let light_count = arrayLength(&point_lights);
    for (var i = 0u; i < light_count; i += 1u) {
        let light = point_lights[i];

        let light_vec = light.position - pos;
        let dist = length(light_vec);

        let atten = attenuation(1., 1., dist, light.radius);

        let light_dir = normalize(light_vec);
        let shade = max(0., dot(normal_tex.rgb, light_dir));
        let diff = light.color * albedo.rgb * shade * atten;

        let refl = reflect(-light_dir, rd);
        let covr = max(0., dot(-rd, nor));
        let spec = light.color * metallic_roughness.z * pow(covr, 16.) * atten;

        color += diff + spec;
    }

    let ltc = ltc_matrix(nor, rd, metallic_roughness.y);
    let area_light_count = arrayLength(&area_lights);
    for (var i = 0u; i < area_light_count; i += 1u) {
        let light = area_lights[i];

        let diff = get_area_light_diffuse(nor, rd, pos, light.points, true);
        let spec = get_area_light_specular(nor, rd, pos, ltc, light.points, true, vec3(0.4, 0.2, 0.1));

        color += light.color * (spec + albedo.rgb * diff);
    }

    return vec4(color, 1.0);
}
