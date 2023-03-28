use std::path::Path;

use color_eyre::Result;
use wgpu::{util::DeviceExt, FilterMode};

use self::conversions::{convert_image_format, convert_sampler};

mod conversions;
pub use conversions::*;

pub struct GltfTexture {
    pub texture: wgpu::Texture,
    pub sampler: Option<usize>,
}

pub struct Gltf {
    pub document: gltf::Document,
    pub buffers: Vec<gltf::buffer::Data>,
    pub gpu_buffers: Vec<wgpu::Buffer>,
    pub images: Vec<gltf::image::Data>,
    pub gpu_textures: Vec<GltfTexture>,
    pub samplers: Vec<wgpu::Sampler>,
    pub default_sampler: wgpu::Sampler,
}

impl Gltf {
    const DEFAULT_SAMPLER_DESC: wgpu::SamplerDescriptor<'static> = wgpu::SamplerDescriptor {
        label: Some("Gltf Default Sampler"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        address_mode_w: wgpu::AddressMode::Repeat,
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        mipmap_filter: FilterMode::Linear,
        lod_min_clamp: 0.0,
        lod_max_clamp: std::f32::MAX,
        compare: None,
        anisotropy_clamp: None,
        border_color: None,
    };

    pub fn import(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        path: impl AsRef<Path>,
    ) -> Result<Self> {
        let (document, buffers, images) = gltf::import(path)?;
        let mut buffer_usages = vec![wgpu::BufferUsages::empty(); buffers.len()];
        for mesh in document.meshes() {
            for primitive in mesh.primitives() {
                if let Some(view) = primitive.indices().and_then(|i| i.view()) {
                    let buffer_index = view.buffer().index();
                    buffer_usages[buffer_index] |= wgpu::BufferUsages::INDEX;
                }
                for (_, accessor) in primitive.attributes() {
                    let Some(buffer_view) = accessor.view() else { continue; };
                    let buffer_index = buffer_view.buffer().index();
                    buffer_usages[buffer_index] |= wgpu::BufferUsages::VERTEX;
                }
            }
        }
        let gpu_buffers = buffers
            .iter()
            .enumerate()
            .map(|(i, buf)| {
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("Gltf Buffer: {i}")),
                    contents: buf,
                    usage: buffer_usages[i],
                })
            })
            .collect();

        let gpu_textures = document
            .textures()
            .map(|t| {
                let image = &images[t.source().index()];
                let texture_desc = wgpu::TextureDescriptor {
                    label: t.name().or(Some("Unnamed Gltf Texture")),
                    size: wgpu::Extent3d {
                        width: image.width,
                        height: image.height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: convert_image_format(image.format),
                    usage: wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                };
                let texture = device.create_texture_with_data(queue, &texture_desc, &image.pixels);
                GltfTexture {
                    texture,
                    sampler: t.sampler().index(),
                }
            })
            .collect();

        let samplers = document
            .samplers()
            .map(|s| convert_sampler(device, s))
            .collect();

        Ok(Self {
            document,
            buffers,
            images,
            gpu_buffers,
            gpu_textures,
            samplers,
            default_sampler: device.create_sampler(&Self::DEFAULT_SAMPLER_DESC),
        })
    }

    fn data_of_accessor<'a>(&'a self, accessor: &gltf::Accessor<'a>) -> Option<&'a [u8]> {
        let buffer_view = accessor.view()?;
        let buffer = buffer_view.buffer();
        let buffer_data = &self.buffers[buffer.index()];
        let buffer_view_data =
            &buffer_data[buffer_view.offset()..buffer_view.offset() + buffer_view.length()];
        let accessor_data = &buffer_view_data
            [accessor.offset()..accessor.offset() + accessor.count() * accessor.size()];
        Some(accessor_data)
    }
}
