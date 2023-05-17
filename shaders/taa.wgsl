#import <utils/uv.wgsl>
#import <utils/color.wgsl>

@group(0) @binding(0) var t_sampler: sampler;
@group(1) @binding(0) var t_input: texture_2d<f32>;
@group(2) @binding(0) var t_history: texture_2d<f32>;
@group(3) @binding(0) var t_motion: texture_2d<f32>;

@group(4) @binding(0) var t_output: texture_storage_2d<rgba16float, write>;

fn mitchell_netravali(x: f32) -> f32 {
    let B = 1.0 / 3.0;
    let C = 1.0 / 3.0;

    let ax = abs(x);
    if ax < 1. {
        return ((12. - 9. * B - 6. * C) * ax * ax * ax + (-18. + 12. * B + 6. * C) * ax * ax + (6. - 2. * B)) / 6.;
    } else if (ax >= 1.) && (ax < 2.) {
        return ((-B - 6. * C) * ax * ax * ax + (6. * B + 30. * C) * ax * ax + (-12. * B - 48. * C) * ax + (8. * B + 24. * C)) / 6.;
    } else {
        return 0.;
    }
}

fn fetch_center_filtered(pix: vec2<i32>) -> vec3<f32> {
    var res = vec4(0.0);

    for (var y = -1; y <= 1; y += 1) {
        for (var x = -1; x <= 1; x += 1) {
            let src = pix + vec2(x, y);
            var col = vec4(textureLoad(t_input, src, 0).rgb, 1.);
            let dist = length(-vec2(f32(x), f32(y)));
            let wt = mitchell_netravali(dist);
            col *= wt;
            res += col;
        }
    }

    return res.rgb / res.a;
}


@compute
@workgroup_size(8, 8, 1)
fn cs_main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let pix = vec2<i32>(global_id.xy);
    let dims = textureDimensions(t_output);
    let uv = get_uv_comp(global_id, dims);

    let velocity = textureLoad(t_motion, pix, 0);
    let history_uv = uv - velocity.xy * 0.5 * vec2(1., -1.);

    var history = textureSampleLevel(t_history, t_sampler, history_uv, 0.).rgb;
    history = rgb_to_ycbcr(history);
    var center = textureLoad(t_input, pix, 0).rgb;
    center = rgb_to_ycbcr(center);

    var vsum = vec3(0.);
    var vsum2 = vec3(0.);
    var wsum = 0.;
    let k = 1;
    for (var y = -k; y <= k; y += 1) {
        for (var x = -k; x <= k; x += 1) {
            var neigh = textureLoad(t_input, pix + vec2(x, y), 0).rgb;
            neigh = rgb_to_ycbcr(neigh);

            let w = exp(-3.0 * f32(x * x + y * y) / f32((k + 1) * (k + 1)));
            vsum += neigh * w;
            vsum2 += neigh * neigh * w;
            wsum += w;
        }
    }

    let ex = vsum / wsum;
    let ex2 = vsum2 / wsum;
    let dev = sqrt(max(vec3(0.0), ex2 - ex * ex));

    let local_contrast = dev.x / (ex.x + 1e-5);

    let history_pixel = history_uv * vec2f(dims);
    let texel_center_dist = dot(vec2(1.0), abs(0.5 - fract(history_pixel)));

    var box_size = 1.0;
    box_size *= mix(0.5, 1.0, smoothstep(-0.1, 0.3, local_contrast));
    box_size *= mix(0.5, 1.0, clamp(1.0 - texel_center_dist, 0.0, 1.0));

    center = fetch_center_filtered(pix);
    center = rgb_to_ycbcr(center);

    let n_deviations = 1.5;
    let nmin = mix(center, ex, box_size * box_size) - dev * box_size * n_deviations;
    let nmax = mix(center, ex, box_size * box_size) + dev * box_size * n_deviations;

    let clamped_history = clamp(history, nmin, nmax);
    var blend_factor = mix(1.0, 1.0 / 12.0, velocity.z);

    let clamp_dist = (min(abs(history.x - nmin.x), abs(history.x - nmax.x))) / max(max(history.x, ex.x), 1e-5);
    blend_factor *= mix(0.2, 1.0, smoothstep(0.0, 2.0, clamp_dist));

    var result = mix(clamped_history, center, blend_factor);
    result = ycbcr_to_rgb(result);

    textureStore(t_output, global_id.xy, vec4(result, 1.));
}
