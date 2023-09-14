#import "shared.wgsl"
#import "encoding.wgsl"
#import "utils/ltc.wgsl"
#import "utils/uv.wgsl"
#import "utils/bvh.wgsl"

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

@group(6) @binding(0) var<storage, read> tlas_nodes: array<TlasNode>;
@group(6) @binding(1) var<storage, read> instances: array<Instance>;
@group(6) @binding(2) var<storage, read> meshes: array<MeshInfo>;
@group(6) @binding(3) var<storage, read> bvh_nodes: array<BvhNode>;
@group(6) @binding(4) var<storage, read> vertices: array<f32>;
@group(6) @binding(5) var<storage, read> indices: array<u32>;

struct Ray2 {
	origin: vec3<f32>,
	dir: vec3<f32>,
}

struct Disk {
	center: vec3<f32>,
	dirx: vec3<f32>,
	diry: vec3<f32>,
	halfx: f32,
	halfy: f32,
	plane: vec4<f32>,
}

fn init_disk(center: vec3<f32>, dirx: vec3<f32>, diry: vec3<f32>, halfx: f32, halfy: f32) -> Disk {
    var disk: Disk;

    disk.center = center;
    disk.dirx = dirx;
    disk.diry = diry;
    disk.halfx = halfx;
    disk.halfy = halfy;

    let normal = cross(dirx, diry);
    disk.plane = vec4(normal, -dot(normal, center));

    return disk;
}

fn ray_plane_intersect(ray: Ray2, plane: vec4<f32>) -> f32 {
    let t = -dot(plane, vec4(ray.origin, 1.0)) / dot(plane.xyz, ray.dir);
    if t > 0.0 {
        return t;
    } else {
        return MAX_DIST;
    }
}

fn init_disk_points(disk: Disk) -> array<vec3<f32>, 4> {
    let ex = disk.halfx * disk.dirx;
    let ey = disk.halfy * disk.diry;

    var points: array<vec3<f32>, 4>;
    points[0] = disk.center - ex - ey;
    points[1] = disk.center + ex - ey;
    points[2] = disk.center + ex + ey;
    points[3] = disk.center - ex + ey;

    return points;
}

fn ray_disc_intersect(ray: Ray2, disk: Disk) -> f32 {
    var t = ray_plane_intersect(ray, disk.plane);
    if t != MAX_DIST {
        let pos = ray.origin + ray.dir * t;
        let lpos = pos - disk.center;

        let x = dot(lpos, disk.dirx);
        let y = dot(lpos, disk.diry);

        let ab = sqr(x / disk.halfx) + sqr(y / disk.halfy);
        if 0.7 > ab || ab > 1.0 {
            t = MAX_DIST;
        }
    }

    return t;
}

// An extended version of the implementation from
// "How to solve a cubic equation, revisited"
// http://momentsingraphics.de/?p=105
fn solve_cubic(coeffs: vec4<f32>) -> vec3<f32> {
	// Normalize the polynomial
    var coeffs = vec4(coeffs.xyz / coeffs.w, coeffs.w);
	// Divide middle coefficients by three
    coeffs.y /= 3.0;
    coeffs.z /= 3.0;

    let A = coeffs.w;
    let B = coeffs.z;
    let C = coeffs.y;
    let D = coeffs.x;

	// Compute the Hessian and the discriminant
    let delta = vec3(-B * B + C, -C * B + D, dot(vec2(B, -C), coeffs.xy));
    let discriminant = dot(vec2(4.0 * delta.x, -delta.y), delta.zy);

    var rootsa: vec3<f32>;
    var rootsb: vec3<f32>;

    var xlc: vec2<f32>;
    var xsc: vec2<f32>;


	// Algorithm A
        {
        let A_a = 1.0;
        let C_a = delta.x;
        let D_a = -2.0 * B * delta.x + delta.y;

		// Take the cubic root of a normalized complex number
        let theta = atan2(sqrt(discriminant), -D_a) / 3.0;

        let x_1a = 2.0 * sqrt(-C_a) * cos(theta);
        let x_3a = 2.0 * sqrt(-C_a) * cos(theta + (2.0 / 3.0) * PI);

        var xl: f32;
        if (x_1a + x_3a) > 2.0 * B {
            xl = x_1a;
        } else {
            xl = x_3a;
        }

        xlc = vec2(xl - B, A);
    }

	// Algorithm D
        {
        let A_d = D;
        let C_d = delta.z;
        let D_d = -D * delta.y + 2.0 * C * delta.z;

		// Take the cubic root of a normalized complex number
        let theta = atan2(D * sqrt(discriminant), -D_d) / 3.0;

        let x_1d = 2.0 * sqrt(-C_d) * cos(theta);
        let x_3d = 2.0 * sqrt(-C_d) * cos(theta + (2.0 / 3.0) * PI);

        var xs: f32;
        if x_1d + x_3d < 2.0 * C {

            xs = x_1d;
        } else {

            xs = x_3d;
        }

        xsc = vec2(-D, xs + C);
    }

    let E = xlc.y * xsc.y;
    let F = -xlc.x * xsc.y - xlc.y * xsc.x;
    let G = xlc.x * xsc.x;

    let xmc = vec2(C * F - B * G, -B * F + C * E);

    var root = vec3(xsc.x / xsc.y, xmc.x / xmc.y, xlc.x / xlc.y);

    if root.x < root.y && root.x < root.z {
        root = root.yxz;
    } else if root.z < root.x && root.z < root.y {
        root = root.xzy;
    }

    return root;
}

fn ltc_evaluate_ring(N: vec3<f32>, V: vec3<f32>, P: vec3<f32>, Minv: mat3x3<f32>, disk: Disk, two_sided: bool) -> vec3<f32> {
    let T1 = normalize(V - N * dot(V, N));
    let T2 = cross(N, T1);

    let points = init_disk_points(disk);

    let R = transpose(mat3x3(T1, T2, N));
    var L_: array<vec3<f32>, 3>;
    L_[0] = R * (points[0] - P);
    L_[1] = R * (points[1] - P);
    L_[2] = R * (points[2] - P);

    var Lo_i = vec3(0.);

    var C = 0.5 * (L_[0] + L_[2]);
    var V1 = 0.5 * (L_[1] - L_[2]);
    var V2 = 0.5 * (L_[1] - L_[0]);

    C = Minv * C;
    V1 = Minv * V1;
    V2 = Minv * V2;

    var occlusion = 1.0;
    if !two_sided && dot(cross(V1, V2), C) < 0.0 {
        occlusion = 0.0;
    }

  // compute eigenvectors of ellipse
    var a: f32; var b: f32;
    let d11 = dot(V1, V1);
    let d22 = dot(V2, V2);
    let d12 = dot(V1, V2);
    if abs(d12) / sqrt(d11 * d22) > 0.0001 {
        let tr = d11 + d22;
        var det = -d12 * d12 + d11 * d22;

    // use sqrt matrix to solve for eigenvalues
        det = sqrt(det);
        let u = 0.5 * sqrt(tr - 2.0 * det);
        let v = 0.5 * sqrt(tr + 2.0 * det);
        let e_max = sqr(u + v);
        let e_min = sqr(u - v);

        var V1_: vec3<f32>;
        var V2_: vec3<f32>;

        if d11 > d22 {
            V1_ = d12 * V1 + (e_max - d11) * V2;
            V2_ = d12 * V1 + (e_min - d11) * V2;
        } else {
            V1_ = d12 * V2 + (e_max - d22) * V1;
            V2_ = d12 * V2 + (e_min - d22) * V1;
        }

        a = 1.0 / e_max;
        b = 1.0 / e_min;
        V1 = normalize(V1_);
        V2 = normalize(V2_);
    } else {
        a = 1.0 / dot(V1, V1);
        b = 1.0 / dot(V2, V2);
        V1 *= sqrt(a);
        V2 *= sqrt(b);
    }

    var V3 = cross(V1, V2);
    if dot(C, V3) < 0.0 {
        V3 *= -1.0;
    }

    let L = dot(V3, C);
    let x0 = dot(V1, C) / L;
    let y0 = dot(V2, C) / L;

    let E1 = inverseSqrt(a);
    let E2 = inverseSqrt(b);

    a *= L * L;
    b *= L * L;

    let c0 = a * b;
    let c1 = a * b * (1.0 + x0 * x0 + y0 * y0) - a - b;
    let c2 = 1.0 - a * (1.0 + x0 * x0) - b * (1.0 + y0 * y0);
    let c3 = 1.0;

    let roots = solve_cubic(vec4(c0, c1, c2, c3));
    let e1 = roots.x;
    let e2 = roots.y;
    let e3 = roots.z;

    var avgDir = vec3(a * x0 / (a - e2), b * y0 / (b - e2), 1.0);

    let rotate = mat3x3(V1, V2, V3);

    avgDir = rotate * avgDir;
    avgDir = normalize(avgDir);

    let L1 = sqrt(-e2 / e3);
    let L2 = sqrt(-e2 / e1);
    let r = sqrt(L1 * L1 + L2 * L2);

    var formFactor = L1 * L2 * inverseSqrt((1.0 + L1 * L1) * (1.0 + L2 * L2));
    // formFactor = r * r / (1. + r * r);
    // let ri = r * 0.9;
    // formFactor = 0.5 * (1. - (r * r - ri * ri + 1.) * inverseSqrt((r * r + ri * ri + 1.) * (r * r + ri * ri + 1.) - 4. * r * r * ri * ri));
    // formFactor = r * r * inverseSqrt(r * r * r * r + 2. * r * r * (2. - 1.) + 1.);
    // formFactor = ri * ri * pow(1. + r * r, 3. / 2.);

	// use tabulated horizon-clipped sphere
    var uv = vec2(avgDir.z * 0.5 + 0.5, formFactor);
    uv = uv * LUT_SCALE + LUT_BIAS;
    let scale = textureSample(texture_array[LTC2_TEXTURE], tex_ltc_sampler, uv).w;

    let spec = formFactor * scale;

    return vec3(spec) * occlusion;
}

fn ltc_evaluate_ring2(N: vec3<f32>, V: vec3<f32>, P: vec3<f32>, Minv: mat3x3<f32>, disk: Disk, two_sided: bool) -> vec3<f32> {
    let r = 0.5;
    let eps = 0.05;
    let sx = disk.halfx * 0.95;
    let sy = disk.halfy * 0.95;
    var disk1 = disk;
    disk1.halfx += clamp(r, eps, sx);
    disk1.halfy += clamp(r, eps, sy);
    let l1 = ltc_evaluate_ring(N, V, P, Minv, disk, two_sided);
    var disk2 = disk;
    disk2.halfx -= clamp(r, eps, sx);
    disk2.halfy -= clamp(r, eps, sy);
    let l2 = ltc_evaluate_ring(N, V, P, Minv, disk2, two_sided);
    return l1 - l2;
}

fn ltc_evaluate_ring3(N: vec3<f32>, V: vec3<f32>, P: vec3<f32>, Minv: mat3x3<f32>, disk: Disk, two_sided: bool) -> vec3<f32> {
    let T1 = normalize(V - N * dot(V, N));
    let T2 = cross(N, T1);

    let points = init_disk_points(disk);

    let R = transpose(mat3x3(T1, T2, N));
    var L_: array<vec3<f32>, 3>;
    L_[0] = R * (points[0] - P);
    L_[1] = R * (points[1] - P);
    L_[2] = R * (points[2] - P);

    var Lo_i = vec3(0.);

    var C = 0.5 * (L_[0] + L_[2]);
    var V1 = 0.5 * (L_[1] - L_[2]);
    var V2 = 0.5 * (L_[1] - L_[0]);

    C = Minv * C;
    V1 = Minv * V1;
    V2 = Minv * V2;

    var occlusion = 1.0;
    if !two_sided && dot(cross(V1, V2), C) < 0.0 {
        occlusion = 0.0;
    }

  // compute eigenvectors of ellipse
    var a: f32; var b: f32;
    let d11 = dot(V1, V1);
    let d22 = dot(V2, V2);
    let d12 = dot(V1, V2);
    if abs(d12) / sqrt(d11 * d22) > 0.0001 {
        let tr = d11 + d22;
        var det = -d12 * d12 + d11 * d22;

    // use sqrt matrix to solve for eigenvalues
        det = sqrt(det);
        let u = 0.5 * sqrt(tr - 2.0 * det);
        let v = 0.5 * sqrt(tr + 2.0 * det);
        let e_max = sqr(u + v);
        let e_min = sqr(u - v);

        var V1_: vec3<f32>;
        var V2_: vec3<f32>;

        if d11 > d22 {
            V1_ = d12 * V1 + (e_max - d11) * V2;
            V2_ = d12 * V1 + (e_min - d11) * V2;
        } else {
            V1_ = d12 * V2 + (e_max - d22) * V1;
            V2_ = d12 * V2 + (e_min - d22) * V1;
        }

        a = 1.0 / e_max;
        b = 1.0 / e_min;
        V1 = normalize(V1_);
        V2 = normalize(V2_);
    } else {
        a = 1.0 / dot(V1, V1);
        b = 1.0 / dot(V2, V2);
        V1 *= sqrt(a);
        V2 *= sqrt(b);
    }

    var V3 = cross(V1, V2);
    if dot(C, V3) < 0.0 {
        V3 *= -1.0;
    }

    let L = dot(V3, C);
    let x0 = dot(V1, C) / L;
    let y0 = dot(V2, C) / L;

    let E1 = inverseSqrt(a);
    let E2 = inverseSqrt(b);

    a *= L * L;
    b *= L * L;

    let c0 = a * b;
    let c1 = a * b * (1.0 + x0 * x0 + y0 * y0) - a - b;
    let c2 = 1.0 - a * (1.0 + x0 * x0) - b * (1.0 + y0 * y0);
    let c3 = 1.0;

    let roots = solve_cubic(vec4(c0, c1, c2, c3));
    var spec1: f32;
        {
        let e1 = roots.x;
        let e2 = roots.y;
        let e3 = roots.z;

        var avgDir = vec3(a * x0 / (a - e2), b * y0 / (b - e2), 1.0);

        let rotate = mat3x3(V1, V2, V3);

        avgDir = rotate * avgDir;
        avgDir = normalize(avgDir);

        var L1 = sqrt(-e2 / e3);
        var L2 = sqrt(-e2 / e1);

        let formFactor = L1 * L2 * inverseSqrt((1.0 + L1 * L1) * (1.0 + L2 * L2));

		// use tabulated horizon-clipped sphere
        var uv = vec2(avgDir.z * 0.5 + 0.5, formFactor);
        uv = uv * LUT_SCALE + LUT_BIAS;
        let scale = textureSample(texture_array[LTC2_TEXTURE], tex_ltc_sampler, uv).w;
        spec1 = formFactor * scale;
    }
    let z = (sin(global.time) * 2.);
    var spec2: f32;
        {
        let e1 = roots.x ;
        let e2 = roots.y ;
        let e3 = roots.z ;

        var avgDir = vec3(a * x0 / (a - e2), b * y0 / (b - e2), 1.0);

        let rotate = mat3x3(V1, V2, V3);

        avgDir = rotate * avgDir;
        avgDir = normalize(avgDir);

        var L1 = sqrt(-e2 / e3) ;
        var L2 = sqrt(-e2 / e1) ;

        let formFactor = L1 * L2 * inverseSqrt((1.0 + L1 * L1) * (1.0 + L2 * L2));

		// use tabulated horizon-clipped sphere
        var uv = vec2(avgDir.z * 0.5 + 0.5, formFactor);
        uv = uv * LUT_SCALE + LUT_BIAS;
        let scale = textureSample(texture_array[LTC2_TEXTURE], tex_ltc_sampler, uv).w;
        spec2 = formFactor * scale;
    }

    let spec = spec2;

    return vec3(spec) * occlusion;
}

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

    var depth = textureLoad(t_depth, load_uv, 0);
    let norm_uv_tex = textureLoad(t_normal_uv, load_uv, 0);
    var material_id = textureLoad(t_material, load_uv, 0).r;

    let material = materials[material_id];
    let uv = unpack2x16float(norm_uv_tex.y);
    let albedo = textureSample(texture_array[material.albedo], t_sampler, uv);
    let emissive = textureSample(texture_array[material.emissive], t_sampler, uv).rgb;
    let metallic_roughness = textureSample(texture_array[material.metallic_roughness], t_sampler, uv);

    let pos = world_position_from_depth(in.uv, depth, camera.clip_to_world);
    let nor = decode_octahedral_32(norm_uv_tex.x);
    let rd = normalize(camera.position.xyz - pos);

    let width = 6.;
    let height = 6.;
    let disk = init_disk(vec3(-3., 3.5, 10.), vec3(1., 0., 0.), vec3(0., 1., 0.), 0.5 * width, 0.5 * height);
    let points = init_disk_points(disk);

    var color = vec3(0.);

    let uuv = in.uv * 2. - 1.0;

    let view_pos = camera.clip_to_world * vec4(uuv, 1., 1.);
    let view_dir = camera.clip_to_world * vec4(uuv, 0., 1.);

    let eye = view_pos.xyz / view_pos.w;
    let dir = normalize(view_dir.xyz);
    let ray = Ray2(pos, rd);
    let disk_hit = ray_disc_intersect(ray, disk);
    if disk_hit < MAX_DIST {
        material_id = LIGHT_MATERIAL;
        color = albedo.rgb + emissive;
        return vec4(color, 1.0);
    }

    color = albedo.rgb * 0.00 + emissive;
    if material_id == 0u {
        return vec4(.13, 0.13, .13, 1.);
    }
    if material_id == LIGHT_MATERIAL {
        color = albedo.rgb + emissive;
        return vec4(color, 1.0);
    }

    let ltc = ltc_matrix(nor, rd, saturate(0.3));
    let two_sided = 0 == 0;
    var sspec = ltc_evaluate_ring(nor, rd, pos, ltc.matrix, disk, two_sided);
    sspec *= vec3(1.) * ltc.t2.x + (1.0 - vec3(1.)) * ltc.t2.y;
    let one = mat3x3(vec3(1., 0., 0.), vec3(0., 1., 0.), vec3(0., 0., 1.));
    var ddiff = ltc_evaluate_ring(nor, rd, pos, one, disk, two_sided);
    color = vec3(1.) * (sspec + vec3(1.) * ddiff);

    color = max(color, vec3(0.));
    return vec4(color, 1.0);
}
