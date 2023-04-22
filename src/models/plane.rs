use glam::{vec4, Vec2, Vec3};

use crate::app::mesh::{BoundingSphere, Mesh};

pub fn plane_mesh(scale: f32) -> Mesh {
    let vertices = [
        [-scale, -scale, scale],
        [scale, -scale, scale],
        [scale, scale, scale],
        [-scale, scale, scale],
    ]
    .map(Vec3::from)
    .to_vec();

    let normals = [
        [0.0, 0.0, 1.0],
        [0.0, 0.0, -1.0],
        [1.0, 0.0, 0.0],
        [-1.0, 0.0, 0.0],
    ]
    .map(Vec3::from)
    .to_vec();
    let tex_coords = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]
        .map(Vec2::from)
        .to_vec();
    let indices = vec![0, 1, 2, 0, 2, 3];
    let tangents = vec![vec4(1., 0., 0., -1.); vertices.len()];

    let bounding_sphere = BoundingSphere {
        center: Vec3::ZERO,
        radius: scale * 2f32.sqrt(),
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
