fn get_uv_comp(global_id: vec3<u32>, tex_size: vec2<u32>) -> vec2<f32> {
    return (vec2<f32>(global_id.xy) + 0.5) / vec2<f32>(tex_size);
}

fn cs_to_uv(cs: vec2<f32>) -> vec2<f32> {
    return cs * vec2(0.5, -0.5) + vec2(0.5, 0.5);
}

fn uv_to_cs(uv: vec2<f32>) -> vec2<f32> {
    return (uv - 0.5) * vec2(2., -2.);
}

fn ndc_from_uv_raw_depth(uv: vec2<f32>, raw_depth: f32) -> vec3<f32> {
    return vec3(uv.x * 2. - 1., (1. - uv.y) * 2. - 1., raw_depth);
}

fn world_position_from_depth(uv: vec2<f32>, raw_depth: f32, inverse_projection_view: mat4x4<f32>) -> vec3<f32> {
    let clip = vec4(ndc_from_uv_raw_depth(uv, raw_depth), 1.0);
    let world_w = inverse_projection_view * clip;

    return world_w.xyz / world_w.w;
}

fn view_position_from_depth(uv: vec2<f32>, raw_depth: f32, inverse_projection: mat4x4<f32>) -> vec3<f32> {
    let clip = vec4(ndc_from_uv_raw_depth(uv, raw_depth), 1.0);
    let world_w = inverse_projection * clip;

    return world_w.xyz / world_w.w;
}

fn raw_depth_to_linear_depth(raw_depth: f32, near: f32, far: f32) -> f32 {
    // NOTE: Vulkan depth is [0, 1]
    return near * far / (far + raw_depth * (near - far));
}

fn linear_depth_to_raw_depth(linear_depth: f32, near: f32, far: f32) -> f32 {
    return (near * far) / (linear_depth * (near - far)) - far / (near - far);
}
