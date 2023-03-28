use image::GenericImageView;
use log::{info, warn};
use std::{
    io::{BufReader, Cursor},
    num::NonZeroU32,
    path::Path,
};
use wgpu::util::DeviceExt;

use crate::model;

pub fn load_texture(
    file_name: impl AsRef<Path>,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> color_eyre::Result<wgpu::Texture> {
    load_texture_path(file_name.as_ref(), device, queue)
}

pub fn load_texture_path(
    file_name: &Path,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> color_eyre::Result<wgpu::Texture> {
    let data = std::fs::read(file_name)?;
    load_texture_from_bytes(
        device,
        queue,
        &data,
        file_name.file_name().and_then(|name| name.to_str()),
    )
}

pub fn load_texture_from_bytes(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    bytes: &[u8],
    label: Option<&str>,
) -> color_eyre::Result<wgpu::Texture> {
    let img = image::load_from_memory(bytes)?;
    load_texture_from_image(device, queue, &img, label)
}

pub fn load_texture_from_image(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    img: &image::DynamicImage,
    label: Option<&str>,
) -> color_eyre::Result<wgpu::Texture> {
    info!("Loading Texture from Image: {label:?}");
    let rgba = img.to_rgba8();
    let (width, height) = img.dimensions();

    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label,
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    queue.write_texture(
        wgpu::ImageCopyTexture {
            aspect: wgpu::TextureAspect::All,
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
        },
        &rgba,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: NonZeroU32::new(4 * width),
            rows_per_image: NonZeroU32::new(height),
        },
        size,
    );

    Ok(texture)
}

pub fn load_model(
    file_name: impl AsRef<Path>,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
) -> color_eyre::Result<model::Model> {
    load_model_path(file_name.as_ref(), device, queue, layout)
}

pub fn load_model_path(
    file_name: &Path,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
) -> color_eyre::Result<model::Model> {
    info!("Loading OBJ Model: {}", file_name.display());
    let parent = file_name.parent().unwrap_or(Path::new("assets"));

    let obj_text = std::fs::read_to_string(file_name)?;
    let obj_cursor = Cursor::new(obj_text);
    let mut obj_reader = BufReader::new(obj_cursor);

    let (models, obj_materials) = tobj::load_obj_buf(
        &mut obj_reader,
        &tobj::LoadOptions {
            triangulate: true,
            single_index: true,
            ..Default::default()
        },
        move |p| {
            let mat_text = std::fs::read_to_string(parent.join(p)).map_err(|err| {
                warn!("Failed to load mtl file with error: {err}");
                tobj::LoadError::OpenFileFailed
            })?;
            tobj::load_mtl_buf(&mut BufReader::new(Cursor::new(mat_text)))
        },
    )?;

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });
    let mut materials = Vec::new();
    for m in obj_materials? {
        let diffuse_texture = load_texture(parent.join(m.diffuse_texture), device, queue)?;
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(
                        &diffuse_texture.create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
            label: None,
        });

        materials.push(model::Material {
            name: m.name,
            diffuse_texture,
            bind_group,
        })
    }

    let meshes = models
        .into_iter()
        .map(|m| {
            let vertices = (0..m.mesh.positions.len() / 3)
                .map(|i| model::ModelVertex {
                    position: [
                        m.mesh.positions[i * 3],
                        m.mesh.positions[i * 3 + 1],
                        m.mesh.positions[i * 3 + 2],
                    ],
                    tex_coords: [m.mesh.texcoords[i * 2], m.mesh.texcoords[i * 2 + 1]],
                    normal: [
                        m.mesh.normals[i * 3],
                        m.mesh.normals[i * 3 + 1],
                        m.mesh.normals[i * 3 + 2],
                    ],
                })
                .collect::<Vec<_>>();

            let file_name_str = file_name.to_str().unwrap_or("Unnamed");
            let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{:?} Vertex Buffer", file_name_str)),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{:?} Index Buffer", file_name_str)),
                contents: bytemuck::cast_slice(&m.mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            model::Mesh {
                name: file_name_str.into(),
                vertex_buffer,
                index_buffer,
                num_elements: m.mesh.indices.len() as u32,
                material: m.mesh.material_id.unwrap_or(0),
            }
        })
        .collect::<Vec<_>>();

    Ok(model::Model { meshes, materials })
}
