use glam::{Vec2, Vec3};

use crate::app::{
    mesh::{BoundingSphere, MeshId},
    App,
};

pub fn plane_mesh(app: &mut App, scale: f32) -> MeshId {
    let vertices = [
        [-scale, -scale, scale],
        [scale, -scale, scale],
        [scale, scale, scale],
        [-scale, scale, scale],
    ]
    .map(Vec3::from);

    let normals = [
        [0.0, 0.0, 1.0],
        [0.0, 0.0, -1.0],
        [1.0, 0.0, 0.0],
        [-1.0, 0.0, 0.0],
    ]
    .map(Vec3::from);
    let tex_coords = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]].map(Vec2::from);
    let indices = [0, 1, 2, 0, 2, 3];

    let bounding_sphere = BoundingSphere {
        center: Vec3::ZERO,
        radius: scale * 2f32.sqrt(),
    };
    app.add_mesh(&vertices, &normals, &tex_coords, &indices, bounding_sphere)
}
