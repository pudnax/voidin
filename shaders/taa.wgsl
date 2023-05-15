@group(0) @binding(0) var t_sampler: sampler;
@group(1) @binding(0) var t_input: texture_2d<f32>;
@group(2) @binding(0) var t_history: texture_2d<f32>;
@group(3) @binding(0) var t_motion: texture_2d<f32>;

@group(4) @binding(0) var t_output: texture_storage_2d<rgba16float, write>;

@compute
@workgroup_size(8, 8, 1)
fn cs_main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let dims = textureDimensions(t_output);
    let uv = vec2<f32>(global_id.xy) / vec2<f32>(dims);
    let prev = textureSampleLevel(t_input, t_sampler, uv, 0.);
    textureStore(t_output, global_id.xy, prev);
}
