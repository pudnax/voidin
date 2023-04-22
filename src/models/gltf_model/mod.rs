use std::{num::NonZeroU32, path::Path, vec};

use ahash::AHashMap;
use color_eyre::{
    eyre::{eyre, Context},
    Result,
};

mod conversions;
pub use conversions::*;
use glam::{Mat4, Vec4};

use crate::{
    app::{
        instance::Instance,
        material::{Material, MaterialId},
        mesh::MeshId,
        texture::TextureId,
        App,
    },
    utils::{mesh_bounding_sphere, FormatConversions, UnwrapRepeat},
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
        let materials = Self::make_materials(app, &document, &images)?;
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

        app.get_texture_pool_mut().update_bind_group();

        Ok(Self {
            document,
            buffers,
            images,
            instances,
        })
    }

    fn make_materials(
        app: &mut App,
        document: &gltf::Document,
        images: &[gltf::image::Data],
    ) -> Result<Vec<MaterialId>> {
        let mut image_map = AHashMap::new();
        let mut encoder = app.device().create_command_encoder(&Default::default());
        let mut materials = vec![];
        for material in document.materials() {
            let name = material.name().unwrap_or("");
            let pbr = material.pbr_metallic_roughness();
            let mut color: Vec4 = pbr.base_color_factor().into();
            color.w = material.alpha_cutoff().unwrap_or(0.5);

            let mut process = |img, srgb| {
                process_texture_cached(app, &mut image_map, images, img, srgb, &mut encoder)
            };

            let albedo = pbr
                .base_color_texture()
                .map(|t| process(t.texture().source(), true))
                .transpose()?
                .unwrap_or(TextureId::default());

            let normal = material
                .normal_texture()
                .map(|t| process(t.texture().source(), false))
                .transpose()?
                .unwrap_or_default();

            let emissive = material
                .emissive_texture()
                .map(|t| process(t.texture().source(), true))
                .transpose()?
                .unwrap_or_default();

            let metallic_roughness = pbr
                .metallic_roughness_texture()
                .map(|t| process(t.texture().source(), false))
                .transpose()?
                .unwrap_or_default();

            let material = Material {
                base_color: color,
                albedo,
                normal,
                metallic_roughness,
                emissive,
            };
            let id = app.get_material_pool_mut().add(material);
            log::info!("Inserted material {name} with id: {:?}", id);
            materials.push(id);
        }

        app.queue().submit(Some(encoder.finish()));

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
                    mesh_bounding_sphere(vertices),
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
        transform: Mat4,
    ) -> Vec<Instance> {
        let mut instances = vec![];

        pub fn traverse_nodes<'a, T>(
            nodes: impl Iterator<Item = gltf::Node<'a>>,
            visitor: &mut impl FnMut(&T, &gltf::Node) -> Option<T>,
            acc: T,
        ) {
            for node in nodes {
                if let Some(res) = visitor(&acc, &node) {
                    traverse_nodes(node.children(), visitor, res);
                }
            }
        }

        traverse_nodes(
            nodes,
            &mut |&parent_transform, node| {
                let transform =
                    parent_transform * Mat4::from_cols_array_2d(&node.transform().matrix());

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

    pub fn node_instances(&self, node: gltf::Node, transform: Option<Mat4>) -> Vec<Instance> {
        let transform = transform.unwrap_or_default()
            * Mat4::from_cols_array_2d(&node.transform().matrix()).inverse();

        self.nodes_data(std::iter::once(node), transform)
    }

    pub fn scene_data(&self, scene: gltf::Scene, transform: Mat4) -> Vec<Instance> {
        self.nodes_data(scene.nodes(), transform)
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

type TexKey = (usize, bool);

fn process_texture_cached(
    app: &mut App,
    image_map: &mut AHashMap<TexKey, TextureId>,
    images: &[gltf::image::Data],
    image: gltf::image::Image<'_>,
    srgb: bool,
    encoder: &mut wgpu::CommandEncoder,
) -> Result<TextureId> {
    let key: TexKey = (image.index(), srgb);

    let entry = match image_map.entry(key) {
        std::collections::hash_map::Entry::Occupied(handle) => return Ok(*handle.get()),
        std::collections::hash_map::Entry::Vacant(v) => v,
    };

    let handle = process_texture(app, images, image, srgb, encoder)?;

    entry.insert(handle);

    Ok(handle)
}

fn process_texture(
    app: &mut App,
    images: &[gltf::image::Data],
    image: gltf::image::Image<'_>,
    srgb: bool,
    encoder: &mut wgpu::CommandEncoder,
) -> Result<TextureId> {
    let name = image.name().unwrap_or("");
    let image = images
        .get(image.index())
        .ok_or_else(|| eyre!("Invalid image index: {}", image.index()))?;
    let (width, height) = (image.width, image.height);
    let (image, format) = convert_to_rgba(image, srgb)?;
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let mip_level_count = size.max_mips(wgpu::TextureDimension::D2);

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
        .generate_mipmaps(encoder, app.device(), &texture);

    let texture_id = app.get_texture_pool_mut().add(texture_view);
    log::info!("Inserted texture {name} with id: {}", texture_id.id());
    Ok(texture_id)
}
