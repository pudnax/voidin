use glam::{vec4, Vec2, Vec3, Vec4};

use crate::{BoundingSphere, Mesh};

pub fn make_plane_mesh(width: f32, height: f32) -> Mesh {
    let width = width / 2.;
    let height = height / 2.;
    let vertices = [
        [-width, -height, 0., 0.],
        [-width, height, 0., 0.],
        [width, height, 0., 0.],
        [width, -height, 0., 0.],
    ]
    .map(Vec4::from)
    .to_vec();

    let normals = [
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
    ]
    .map(Vec4::from)
    .to_vec();
    let tex_coords = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]
        .map(Vec2::from)
        .to_vec();
    let indices = vec![0, 1, 2, 0, 2, 3];
    let tangents = vec![vec4(1., 0., 0., -1.); vertices.len()];

    let bounding_sphere = BoundingSphere {
        center: Vec3::ZERO,
        radius: Vec2::new(width, height).length(),
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
