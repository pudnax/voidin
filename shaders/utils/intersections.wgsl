#import <./math.wgsl>

struct Ray {
    eye: vec3<f32>,
	dir: vec3<f32>,
	inv_dir: vec3<f32>,
}

fn ray_new(eye: vec3<f32>, dir: vec3<f32>) -> Ray {
    return Ray(eye, dir, 1. / dir);
}

fn intersect_aabb(ray: Ray, bmin: vec3<f32>, bmax: vec3<f32>, t: f32) -> f32 {
    let tx1 = (bmin - ray.eye) * ray.inv_dir;
    let tx2 = (bmax - ray.eye) * ray.inv_dir;
    let tmax = min_element(max(tx1, tx2));
    let tmin = max_element(min(tx1, tx2));
    if tmax >= tmin && tmin < t && tmax > 0. {
        return tmin;
    } else {
        return MAX_DIST;
    }
}

fn intersect_trig(ray: Ray, v0: vec3<f32>, v1: vec3<f32>, v2: vec3<f32>, hit: ptr<function,f32>) -> bool {
    let edge1 = v1 - v0;
    let edge2 = v2 - v0;
    let uvec = cross(ray.dir, edge2);
    let det = dot(edge1, uvec);
    if det < 1e-10 { return false; } // cull backface
    let inv_det = 1. / det;
    let orig = ray.eye - v0;
    let u = inv_det * dot(orig, uvec);
    if u < 0. || 1. < u { return false; }
    let vvec = cross(orig, edge1);
    let v = inv_det * dot(ray.dir, vvec);
    if v < 0. || u + v > 1. { return false; }
    let t = inv_det * dot(edge2, vvec);
    if t > 0.0 && t < *hit {
        *hit = t;
        return true;
    } else {
        return false;
    }
}
