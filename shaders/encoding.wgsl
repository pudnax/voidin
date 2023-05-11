const PRES = 16u;

// https://www.shadertoy.com/view/4llcRl
fn encode_octahedral_32(nor: vec3<f32>) -> u32 {
    var nor = nor / (abs(nor.x) + abs(nor.y) + abs(nor.z));
    if nor.z < 0.0 {
        let xy = (1.0 - abs(nor.yx)) * sign(nor.xy);
        nor = vec3(xy, nor.z);
    }
    let v = nor.xy * 0.5 + 0.5;

    let mu = (1u << PRES) - 1u;
    let d = vec2<u32>(floor(v * f32(mu) + 0.5));
    return (d.y << PRES) | d.x;
}

fn decode_octahedral_32(data: u32) -> vec3<f32> {
    let mu = (1u << PRES) - 1u;
    let d = vec2<u32>(data, data >> PRES) & mu;
    var v = vec2<f32>(d) / f32(mu);

    v = v * 2.0 - 1.0;
    var nor = vec3(v, 1.0 - abs(v.x) - abs(v.y));
    let t = max(-nor.z, 0.0);
    if nor.x > 0.0 { nor.x += -t; } else { nor.x += t; }
    if nor.y > 0.0 { nor.y += -t; } else { nor.y += t; }
    return normalize(nor);
}
