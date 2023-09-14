use glam::{vec4, Vec2, Vec3};

use crate::Mesh;

pub fn make_cube_mesh(scale: f32) -> Mesh {
    let vertices = [
        // Front face
        [-scale, -scale, scale],
        [scale, -scale, scale],
        [scale, scale, scale],
        [-scale, scale, scale],
        // Back face
        [-scale, -scale, -scale],
        [-scale, scale, -scale],
        [scale, scale, -scale],
        [scale, -scale, -scale],
        // Top face
        [-scale, scale, -scale],
        [-scale, scale, scale],
        [scale, scale, scale],
        [scale, scale, -scale],
        // Bottom face
        [-scale, -scale, -scale],
        [scale, -scale, -scale],
        [scale, -scale, scale],
        [-scale, -scale, scale],
        // Right face
        [scale, -scale, -scale],
        [scale, scale, -scale],
        [scale, scale, scale],
        [scale, -scale, scale],
        // Left face
        [-scale, -scale, -scale],
        [-scale, -scale, scale],
        [-scale, scale, scale],
        [-scale, scale, -scale],
    ]
    .map(Vec3::from)
    .to_vec();
    let normals = [
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, -1.0],
        [0.0, 0.0, -1.0],
        [0.0, 0.0, -1.0],
        [0.0, 0.0, -1.0],
        [0.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, -1.0, 0.0],
        [0.0, -1.0, 0.0],
        [1.0, -1.0, 0.0],
        [0.0, -1.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [-1.0, 0.0, 0.0],
        [-1.0, 0.0, 0.0],
        [-1.0, 0.0, 0.0],
        [-1.0, 0.0, 0.0],
    ]
    .map(Vec3::from)
    .to_vec();
    let tex_coords = [
        [0.0, 0.0],
        [1.0, 0.0],
        [1.0, 1.0],
        [0.0, 1.0],
        [0.0, 0.0],
        [1.0, 0.0],
        [1.0, 1.0],
        [0.0, 1.0],
        [0.0, 0.0],
        [1.0, 0.0],
        [1.0, 1.0],
        [0.0, 1.0],
        [0.0, 0.0],
        [1.0, 0.0],
        [1.0, 1.0],
        [0.0, 1.0],
        [0.0, 0.0],
        [1.0, 0.0],
        [1.0, 1.0],
        [0.0, 1.0],
        [0.0, 0.0],
        [1.0, 0.0],
        [1.0, 1.0],
        [0.0, 1.0],
    ]
    .map(Vec2::from)
    .to_vec();
    let indices = vec![
        0, 1, 2, 0, 2, 3, // front
        4, 5, 6, 4, 6, 7, // back
        8, 9, 10, 8, 10, 11, // top
        12, 13, 14, 12, 14, 15, // bottom
        16, 17, 18, 16, 18, 19, // right
        20, 21, 22, 20, 22, 23, // left
    ];
    let tangents = vec![vec4(1., 0., 0., -1.); vertices.len()];

    Mesh {
        vertices,
        normals,
        tangents,
        tex_coords,
        indices,
    }
}
