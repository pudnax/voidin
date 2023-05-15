use glam::{vec2, vec3, vec4, Vec3};
use std::f32::consts::PI;

use crate::app::mesh::{BoundingSphere, Mesh};

pub fn make_uv_sphere(radius: f32, resolution: usize) -> Mesh {
    let vside = 4 * resolution; // stack
    let uside = vside * 2; // sector

    let mut vertices = Vec::with_capacity((uside + 1) * (vside + 1));
    let mut normals = Vec::with_capacity(vertices.len());
    let mut tangents = Vec::with_capacity(vertices.len());
    let mut uv = Vec::with_capacity(vertices.len());

    let tangent = vec4(1., 0., 0., -1.);

    for v in (0..=vside).map(|v| (v as f32 / vside as f32)) {
        for u in (0..=uside).map(|u| u as f32 / uside as f32) {
            let theta = 2. * PI * u + PI;
            let phi = PI * v;

            let x = theta.cos() * phi.sin() * radius;
            let y = -phi.cos() * radius;
            let z = theta.sin() * phi.sin() * radius;

            let vertex = vec3(x, y, z);
            vertices.push(vertex);
            normals.push(vertex.normalize());
            tangents.push(tangent);
            uv.push(vec2(u, v));
        }
    }

    let stack_count = vside as u32;
    let sector_count = uside as u32;
    let mut indices = Vec::with_capacity(uside * vside * 6);
    // We create a triangle strip as we loop, with `k1` being the top vertices
    // and `k2` being the bottom vertices.
    //  k1--k1+1
    //  |  / |
    //  | /  |
    //  k2--k2+1
    for (i, k1) in (0..=stack_count).map(|i| (i, i * (sector_count + 1))) {
        // k1 - top row; k2 - bottom row
        for (k1, k2) in (0..sector_count).map(|j| (j + k1, j + k1 + sector_count + 1)) {
            if i != 0 {
                indices.push(k1);
                indices.push(k2);
                indices.push(k1 + 1);
            }

            if i != stack_count {
                indices.push(k1 + 1);
                indices.push(k2);
                indices.push(k2 + 1);
            }
        }
    }

    let bounding_sphere = BoundingSphere {
        center: Vec3::ZERO,
        radius,
    };
    Mesh {
        vertices,
        normals,
        tangents,
        tex_coords: uv,
        indices,
        bounding_sphere,
    }
}
