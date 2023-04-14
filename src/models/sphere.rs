use glam::{vec2, vec3};
use std::f32::consts::PI;

use crate::app::{mesh::MeshId, App};

pub fn sphere_mesh(app: &mut App, radius: f32, sector_count: u32, stack_count: u32) -> MeshId {
    let mut vertices = vec![];
    let mut tex_coords = vec![];
    let mut normals = vec![];
    let mut indices = vec![];

    // Vertex positions
    let (mut x, mut y, mut z, mut xy): (f32, f32, f32, f32);
    // Normals
    let (mut nx, mut ny, mut nz): (f32, f32, f32);
    let length_inv = 1.0 / radius;
    // Texture Coordinates
    let (mut s, mut t): (f32, f32);

    let sector_step = 2.0 * PI / sector_count as f32;
    let stack_step = PI / stack_count as f32;
    let (mut sector_angle, mut stack_angle): (f32, f32);

    // Build the vertex buffer data (positioin, normal, tex coords)
    for i in 0..(stack_count + 1) {
        stack_angle = PI / 2.0 - i as f32 * stack_step;
        xy = radius * stack_angle.cos();
        z = radius * stack_angle.sin();

        for j in 0..(sector_count + 1) {
            sector_angle = j as f32 * sector_step;

            // Vertices
            x = xy * sector_angle.cos();
            y = xy * sector_angle.sin();

            // Normals
            nx = x * length_inv;
            ny = y * length_inv;
            nz = z * length_inv;

            // Texture coordinates
            s = (j / sector_count) as f32;
            t = (i / stack_count) as f32;

            vertices.push(vec3(x, y, z));
            normals.push(vec3(nx, ny, nz));
            tex_coords.push(vec2(s, t));
        }
    }

    // Build index buffer
    let (mut k1, mut k2): (u32, u32);

    // We create a triangle strip as we loop, with `k1` being the top vertices
    // and `k2` being the bottom vertices.
    //  k1--k1+1
    //  |  / |
    //  | /  |
    //  k2--k2+1
    for i in 0..(stack_count + 1) {
        // Top row
        k1 = i * (sector_count + 1);
        // Bottom row
        k2 = k1 + (sector_count + 1);

        for _j in 0..(sector_count + 1) {
            if i != 0 {
                indices.push(k1);
                indices.push(k2);
                indices.push(k1 + 1);
            }

            if i != (stack_count - 1) {
                indices.push(k1 + 1);
                indices.push(k2);
                indices.push(k2 + 1);
            }

            k1 += 1;
            k2 += 1;
        }
    }

    app.add_mesh(&vertices, &normals, &tex_coords, &indices)
}
