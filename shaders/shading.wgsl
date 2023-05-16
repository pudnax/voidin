#import <shared.wgsl>
#import <encoding.wgsl>
#import <utils/ltc.wgsl>
#import <utils/uv.wgsl>

@group(0) @binding(0) var<uniform> global: Globals;
@group(0) @binding(1) var<uniform> camera: Camera;

@group(1) @binding(0) var t_normal_uv: texture_2d<u32>;
@group(1) @binding(1) var t_material: texture_2d<u32>;
@group(1) @binding(2) var t_depth: texture_depth_2d;
@group(1) @binding(3) var t_sampler: sampler;

@group(2) @binding(0) var texture_array: binding_array<texture_2d<f32>>;
@group(2) @binding(1) var tex_sampler: sampler;
@group(2) @binding(2) var tex_ltc_sampler: sampler;

@group(3) @binding(0) var<storage, read> materials: array<Material>;

@group(4) @binding(0) var<storage, read> point_lights: array<Light>;
@group(5) @binding(0) var<storage, read> area_lights: array<AreaLight>;

struct VertexOutput {
  @builtin(position) pos: vec4<f32>,
  @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_idx: u32) -> VertexOutput {
    var out: VertexOutput;
    out.uv = vec2<f32>(vec2((vertex_idx << 1u) & 2u, vertex_idx & 2u));
    out.pos = vec4(2.0 * out.uv.x - 1.0, 1. - out.uv.y * 2., 0.0, 1.0);
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
    let tex_dims = vec2f(textureDimensions(t_normal_uv));
    let load_uv = vec2<u32>(in.uv * tex_dims);

    let depth = textureLoad(t_depth, load_uv, 0);
    let norm_uv_tex = textureLoad(t_normal_uv, load_uv, 0);
    let material_id = textureLoad(t_material, load_uv, 0).r;

    let material = materials[material_id];
    let uv = unpack2x16float(norm_uv_tex.y);
    let albedo = textureSample(texture_array[material.albedo], t_sampler, uv);
    let emissive = textureSample(texture_array[material.emissive], t_sampler, uv).rgb;
    let metallic_roughness = textureSample(texture_array[material.metallic_roughness], t_sampler, uv);

    let pos = world_position_from_depth(in.uv, depth, camera.inv_proj_view);
    let nor = decode_octahedral_32(norm_uv_tex.x);
    let rd = normalize(camera.position.xyz - pos);

    var color = vec3(0.);

    color = albedo.rgb * 0.01 + emissive;
    if material_id == LIGHT_MATERIAL {
        color = albedo.rgb + emissive;
    }

    let light_count = arrayLength(&point_lights);
    for (var i = 0u; i < light_count; i += 1u) {
        if material_id == LIGHT_MATERIAL { break; }

        let light = point_lights[i];

        let light_vec = light.position - pos;
        let dist = length(light_vec);
        if dist - light.radius > 0. { continue; }

        let atten = attenuation(1., 1., dist, light.radius);

        let light_dir = normalize(light_vec);
        let shade = max(0., dot(nor, light_dir));
        let diff = light.color * albedo.rgb * shade * atten;

        let refl = reflect(-light_dir, rd);
        let covr = max(0., dot(-rd, nor));
        let spec = light.color * metallic_roughness.z * pow(covr, 16.) * atten;

        color += diff + spec;
    }

    let ltc = ltc_matrix(nor, rd, saturate(metallic_roughness.x));
    let area_light_count = arrayLength(&area_lights);
    for (var i = 0u; i < area_light_count; i += 1u) {
        if material_id == LIGHT_MATERIAL { break; }
        let light_radius = 25.;

        let light = area_lights[i];
        let center = mix(light.points[0], light.points[2], 0.5);
        let light_vec = center - pos;
        let dist = length(light_vec);

        let diff = get_area_light_diffuse(nor, rd, pos, light.points, false);
        let spec = get_area_light_specular(nor, rd, pos, ltc, light.points, false, vec3(1.));

        let atten = attenuation(light.intensity, 500., distance(center, pos), light_radius);
        color += light.color * light.intensity * (spec * atten + albedo.rgb * diff) ;
    }

    color = max(color, vec3(0.));
    return vec4(color, 1.0);
}
