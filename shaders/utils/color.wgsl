fn calculate_luma(col: vec3<f32>) -> f32 {
    return dot(vec3(0.2126, 0.7152, 0.0722), col);
}

fn rgb_to_ycbcr(col: vec3<f32>) -> vec3<f32> {
    let m = mat3x3(0.2126, 0.7152, 0.0722, -0.1146, -0.3854, 0.5, 0.5, -0.4542, -0.0458);
    return col * m;
}

fn ycbcr_to_rgb(col: vec3<f32>) -> vec3<f32> {
    let m = mat3x3(1.0, 0.0, 1.5748, 1.0, -0.1873, -.4681, 1.0, 1.8556, 0.0);
    return col * m;
}
