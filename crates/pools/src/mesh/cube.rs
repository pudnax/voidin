use glam::{vec4, Vec2, Vec3, Vec4};

use crate::{BoundingSphere, Mesh};

pub fn make_cube_mesh(scale: f32) -> Mesh {
    let vertices = [
        // Front face
        [-scale, -scale, scale, 0.],
        [scale, -scale, scale, 0.],
        [scale, scale, scale, 0.],
        [-scale, scale, scale, 0.],
        // Back face
        [-scale, -scale, -scale, 0.],
        [-scale, scale, -scale, 0.],
        [scale, scale, -scale, 0.],
        [scale, -scale, -scale, 0.],
        // Top face
        [-scale, scale, -scale, 0.],
        [-scale, scale, scale, 0.],
        [scale, scale, scale, 0.],
        [scale, scale, -scale, 0.],
        // Bottom face
        [-scale, -scale, -scale, 0.],
        [scale, -scale, -scale, 0.],
        [scale, -scale, scale, 0.],
        [-scale, -scale, scale, 0.],
        // Right face
        [scale, -scale, -scale, 0.],
        [scale, scale, -scale, 0.],
        [scale, scale, scale, 0.],
        [scale, -scale, scale, 0.],
        // Left face
        [-scale, -scale, -scale, 0.],
        [-scale, -scale, scale, 0.],
        [-scale, scale, scale, 0.],
        [-scale, scale, -scale, 0.],
    ]
    .map(Vec4::from)
    .to_vec();
    let normals = [
        [0.0, 0.0, 1.0, 0.],
        [0.0, 0.0, -1.0, 0.],
        [1.0, 0.0, 0.0, 0.],
        [-1.0, 0.0, 0.0, 0.],
        [0.0, 1.0, 0.0, 0.],
        [0.0, -1.0, 0.0, 0.],
        [0.0, 0.0, 1.0, 0.],
        [0.0, 0.0, -1.0, 0.],
        [1.0, 0.0, 0.0, 0.],
        [-1.0, 0.0, 0.0, 0.],
        [0.0, 1.0, 0.0, 0.],
        [0.0, -1.0, 0.0, 0.],
        [0.0, 0.0, 1.0, 0.],
        [0.0, 0.0, -1.0, 0.],
        [1.0, 0.0, 0.0, 0.],
        [-1.0, 0.0, 0.0, 0.],
        [0.0, 1.0, 0.0, 0.],
        [0.0, -1.0, 0.0, 0.],
        [0.0, 0.0, 1.0, 0.],
        [0.0, 0.0, -1.0, 0.],
        [1.0, 0.0, 0.0, 0.],
        [-1.0, 0.0, 0.0, 0.],
        [0.0, 1.0, 0.0, 0.],
        [0.0, -1.0, 0.0, 0.],
    ]
    .map(Vec4::from)
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

    let bounding_sphere = BoundingSphere {
        center: Vec3::ZERO,
        radius: scale * 3f32.sqrt(),
    };
    Mesh {
        vertices,
        normals,
        tangents,
        tex_coords,
        indices,
        bounding_sphere,
    }
}
