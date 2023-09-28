#import "shared.wgsl"

const EPS = 0.0001;

fn nmod(a: f32, b: f32) -> f32 {
    var m = a % b;
    if m < 0.0 {
        if b < 0.0 {
            m -= b;
        } else {
            m += b;
        }
    }
    return m;
}

fn nmod3(a: vec3<f32>, b: f32) -> vec3<f32> {
    return vec3(nmod(a.x, b), nmod(a.y, b), nmod(a.z, b));
}

@group(0) @binding(0) var<uniform> global: Globals;
@group(0) @binding(1) var<uniform> camera: Camera;

struct VertexOutput {
  @builtin(position) pos: vec4<f32>,
  @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main_trig(@builtin(vertex_index) vertex_idx: u32) -> VertexOutput {
    let uv = vec2<f32>(vec2((vertex_idx << 1u) & 2u, vertex_idx & 2u));
    let out = VertexOutput(vec4(2.0 * uv - 1.0, 0.0, 1.0), uv);
    return out;
}

fn map(pos: vec3<f32>) -> f32 {
    var p = pos;
    var s = 3.0f;
    for (var i = 0; i < 8; i += 1) {
        p = (nmod3(p - 1.0f, 2.)) - 1.0f;
        let e = 1.4f / dot(p, p);
        s *= e;
        p *= e;
    }
    return length(p.yz) / s;
}

fn get_nor(p: vec3<f32>) -> vec3<f32> {
    let k = mat3x3(p, p, p) - mat3x3(EPS, 0., 0., 0., EPS, 0., 0., 0., EPS);
    return normalize(map(p) - vec3(map(k[0]), map(k[1]), map(k[2])));
}

fn trace(ro: vec3<f32>, rd: vec3<f32>) -> vec3<f32> {
    var hit = -1;
    var t = 0.001;
    var n = 0.;
    for (var i = 0; i < 300; i += 1) {
        let d = map(ro + rd * t);
        if abs(d) < 0.0001 { break; }
        t += d;
        n += 1.;
        if t > 100. { return vec3(1e9, -1., 0.); }
    }
    return vec3(t, 1., n);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let aspect = global.resolution.y / global.resolution.x;
    var uv = (in.uv - 0.5) * aspect;

    let view_pos = camera.clip_to_world * vec4(uv, 1., 1.);
    let view_dir = camera.clip_to_world * vec4(uv, 0., 1.);

    let ro = view_pos.xyz / view_pos.w;
    let rd = normalize(view_dir.xyz);

    // let ro = vec3(0., 0., -3.);
    // let rd = normalize(vec3(uv, 1.));

    var color = vec3(0.13);

    let res = trace(ro, rd);

    if res.y > 0. {
        let pos = ro + rd * res.x;
        // let nor = get_nor(pos);
        // color = nor * 0.5 + 0.5;
        color = vec3(res.z / 200.);
    }

    return vec4(color, 1.);
}
