mod cube;
mod gltf_model;
mod plane;
mod sphere;

use color_eyre::Result;
use glam::Vec3;
use std::path::Path;

pub use cube::cube_mesh;
pub use gltf_model::*;
pub use plane::plane_mesh;
pub use sphere::sphere_mesh;

use crate::app::{
    material::{Material, MaterialId},
    mesh::MeshId,
    App,
};

pub struct ObjModel;

impl ObjModel {
    pub fn import(app: &mut App, path: impl AsRef<Path>) -> Result<Vec<(MeshId, MaterialId)>> {
        let (model_meshes, model_materials) =
            tobj::load_obj(path.as_ref(), &tobj::GPU_LOAD_OPTIONS)?;

        let mut materials = vec![];
        if let Ok(model_materials) = model_materials {
            for material in model_materials {
                let base_color = Vec3::from_array(material.diffuse);
                let material_id = app.add_material(Material {
                    base_color: base_color.extend(0.5),
                    ..Default::default()
                });
                materials.push(material_id);
            }
        }

        let mut meshes = vec![];
        for mesh in model_meshes.iter().map(|m| &m.mesh) {
            let mesh_id = app.add_mesh(
                bytemuck::cast_slice(&mesh.positions),
                bytemuck::cast_slice(&mesh.normals),
                bytemuck::cast_slice(&mesh.texcoords),
                bytemuck::cast_slice(&mesh.indices),
            );
            let material_id = match mesh.material_id {
                Some(id) => materials[id],
                None => MaterialId::default(),
            };
            meshes.push((mesh_id, material_id));
        }

        app.textures_bind_group = app.texture_manager.create_bind_group(app.device());
        Ok(meshes)
    }
}