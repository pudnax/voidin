use std::{num::NonZeroU32, path::Path, vec};

use color_eyre::{
    eyre::{eyre, Context},
    Result,
};

mod conversions;
pub use conversions::*;
use glam::Vec4;

use crate::{
    app::{
        instance::Instance,
        material::{Material, MaterialId},
        mesh::MeshId,
        texture::TextureId,
        App,
    },
    utils::{FormatConversions, UnwrapRepeat},
};

pub struct GltfDocument {
    pub document: gltf::Document,
    pub buffers: Vec<gltf::buffer::Data>,
    pub images: Vec<gltf::image::Data>,

    pub instances: Vec<Vec<Instance>>,
}

impl GltfDocument {
    pub fn import(app: &mut App, path: impl AsRef<Path>) -> Result<Self> {
        let name = path.as_ref().file_name();
        log::info!("Started processing model: {name:?}",);
        let (document, buffers, images) = gltf::import(&path)
            .with_context(|| eyre!("Failed to open file: {}", path.as_ref().display()))?;
        let textures = Self::make_textures(app, &document, &images)?;
        let materials = Self::make_materials(app, &document, &textures)?;
        let meshes = Self::make_meshes(app, &document, &buffers)?;

        let mut instances = vec![];
        for (mesh, mesh_ids) in document.meshes().zip(&meshes) {
            let instance: Vec<_> = mesh
                .primitives()
                .zip(mesh_ids)
                .map(|(primitive, &mesh_id)| {
                    let material_id = primitive
                        .material()
                        .index()
                        .and_then(|index| materials.get(index).copied())
                        .unwrap_or_default();

                    Instance {
                        mesh: mesh_id,
                        material: material_id,
                        ..Default::default()
                    }
                })
                .collect();
            instances.push(instance);
        }

        app.get_texture_manager_mut().update_bind_group();

        Ok(Self {
            document,
            buffers,
            images,
            instances,
        })
    }

    fn make_textures(
        app: &mut App,
        document: &gltf::Document,
        images: &[gltf::image::Data],
    ) -> Result<Vec<TextureId>> {
        let mut encoder = app.device().create_command_encoder(&Default::default());
        let mut textures = vec![];
        for image in document.images() {
            let name = image.name().unwrap_or("");
            let image = images
                .get(image.index())
                .ok_or_else(|| eyre!("Invalid image index"))?;
            let (width, height) = (image.width, image.height);
            let image = convert_to_rgba(image)?;
            let size = wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            };
            let mip_level_count = size.max_mips(wgpu::TextureDimension::D2);

            let format = wgpu::TextureFormat::Rgba8Unorm;
            let desc = wgpu::TextureDescriptor {
                label: None,
                size,
                mip_level_count,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::RENDER_ATTACHMENT,

                view_formats: &[format, format.swap_srgb_suffix()],
            };
            let texture = app.device().create_texture(&desc);
            app.queue().write_texture(
                wgpu::ImageCopyTextureBase {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                image.as_raw(),
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: NonZeroU32::new(width * 4),
                    rows_per_image: None,
                },
                size,
            );
            let texture_view = texture.create_view(&Default::default());

            app.blitter
                .generate_mipmaps(&mut encoder, app.device(), &texture);

            let texture_id = app.get_texture_manager_mut().add(texture_view);
            log::info!("Inserted texture {name} with id: {}", texture_id.id());
            textures.push(texture_id);
        }

        app.queue().submit(Some(encoder.finish()));

        document
            .textures()
            .map(|texture| {
                textures
                    .get(texture.source().index())
                    .copied()
                    .ok_or_else(|| eyre!("Invalid texture image index"))
            })
            .collect()
    }

    fn make_materials(
        app: &mut App,
        document: &gltf::Document,
        textures: &[TextureId],
    ) -> Result<Vec<MaterialId>> {
        let mut materials = vec![];
        for material in document.materials() {
            let name = material.name().unwrap_or("");
            let pbr = material.pbr_metallic_roughness();
            let mut color: Vec4 = pbr.base_color_factor().into();
            color.w = material.alpha_cutoff().unwrap_or(0.5);

            let albedo = pbr
                .base_color_texture()
                .and_then(|t| textures.get(t.texture().index()).copied())
                .unwrap_or_default();

            let normal = material
                .normal_texture()
                .and_then(|t| textures.get(t.texture().index()).copied())
                .unwrap_or_default();

            let metallic_roughness = material
                .pbr_metallic_roughness()
                .metallic_roughness_texture()
                .and_then(|t| textures.get(t.texture().index()).copied())
                .unwrap_or_default();

            let emissive = material
                .emissive_texture()
                .and_then(|t| textures.get(t.texture().index()).copied())
                .unwrap_or_default();

            let material = Material {
                base_color: color,
                albedo,
                normal,
                metallic_roughness,
                emissive,
            };
            let id = app.get_material_manager_mut().add(material);
            log::info!("Inserted material {name} with id: {:?}", id);
            materials.push(id);
        }

        Ok(materials)
    }

    fn make_meshes(
        app: &mut App,
        document: &gltf::Document,
        buffers: &[gltf::buffer::Data],
    ) -> Result<Vec<Vec<MeshId>>> {
        let mut meshes = vec![];
        for mesh in document.meshes() {
            let mut primitives = vec![];
            for primitive in mesh.primitives() {
                let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
                let get_data = |semantic: &gltf::Semantic| -> Option<&[u8]> {
                    primitive
                        .get(semantic)
                        .and_then(|sem| data_of_accessor(buffers, &sem))
                };
                let Some(vertices) = get_data(&gltf::Semantic::Positions) else { continue; };
                let zeros = vec![0u8; vertices.len()];
                let vertices = bytemuck::cast_slice(vertices);
                let normals = get_data(&gltf::Semantic::Normals).unwrap_or(&zeros);
                let tex_coords: Vec<[f32; 2]> = reader
                    .read_tex_coords(0)
                    .map(|uv| uv.into_f32())
                    .unwrap_repeat()
                    .take(vertices.len())
                    .collect();
                let indices: Vec<_> = match reader.read_indices() {
                    Some(indices) => indices.into_u32().collect(),
                    None => (0..vertices.len() as u32).collect(),
                };
                let mesh = app.add_mesh(
                    vertices,
                    bytemuck::cast_slice(normals),
                    bytemuck::cast_slice(&tex_coords),
                    &indices,
                );
                primitives.push(mesh);
            }
            meshes.push(primitives);
        }

        Ok(meshes)
    }

    fn nodes_data<'a>(
        &self,
        nodes: impl Iterator<Item = gltf::Node<'a>>,
        transform: glam::Mat4,
    ) -> Vec<Instance> {
        let mut instances = vec![];

        traverse_nodes_tree(
            nodes,
            &mut |parent_transform, node| {
                let transform =
                    *parent_transform * glam::Mat4::from_cols_array_2d(&node.transform().matrix());

                let mesh_instances = node
                    .mesh()
                    .and_then(|mesh| self.instances.get(mesh.index()));
                if let Some(mesh_instances) = mesh_instances {
                    instances.extend(mesh_instances.iter().map(|&instance| Instance {
                        transform,
                        ..instance
                    }))
                }

                Some(transform)
            },
            transform,
        );

        instances
    }

    pub fn node_instances(&self, node: gltf::Node, transform: Option<glam::Mat4>) -> Vec<Instance> {
        let transform = transform.unwrap_or_default()
            * glam::Mat4::from_cols_array_2d(&node.transform().matrix()).inverse();

        self.nodes_data(std::iter::once(node), transform)
    }

    pub fn scene_data(&self, scene: gltf::Scene, transform: glam::Mat4) -> Vec<Instance> {
        self.nodes_data(scene.nodes(), transform)
    }

    pub fn scene_instances(
        &self,
        scene_name: Option<&str>,
        transform: Option<glam::Mat4>,
    ) -> Option<Vec<Instance>> {
        let scene = if let Some(scene_name) = scene_name {
            self.document
                .scenes()
                .find(|scene| scene.name() == Some(scene_name))?
        } else {
            self.document.default_scene()?
        };

        Some(self.scene_data(scene, transform.unwrap_or_default()))
    }

    pub fn get_node(&self, name: &str) -> Option<gltf::Node> {
        self.document.nodes().find(|node| node.name() == Some(name))
    }
}

pub fn data_of_accessor<'a>(
    buffers: &'a [gltf::buffer::Data],
    accessor: &gltf::Accessor<'a>,
) -> Option<&'a [u8]> {
    let buffer_view = accessor.view()?;
    let buffer = buffer_view.buffer();
    let buffer_data = &buffers[buffer.index()];
    let buffer_view_data = &buffer_data[buffer_view.offset()..][..buffer_view.length()];
    let accessor_data =
        &buffer_view_data[accessor.offset()..][..accessor.count() * accessor.size()];
    Some(accessor_data)
}

pub fn traverse_nodes_tree<'a, T>(
    nodes: impl Iterator<Item = gltf::Node<'a>>,
    visitor: &mut dyn FnMut(&T, &gltf::Node) -> Option<T>,
    acc: T,
) {
    for node in nodes {
        if let Some(res) = visitor(&acc, &node) {
            traverse_nodes_tree(node.children(), visitor, res);
        }
    }
}
