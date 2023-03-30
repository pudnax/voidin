use std::mem::size_of;

use gltf::{
    accessor::{
        DataType,
        Dimensions::{Mat2, Mat3, Mat4, Scalar, Vec2, Vec3, Vec4},
    },
    texture::{MagFilter, MinFilter},
};
use wgpu::{FilterMode, PrimitiveTopology, TextureFormat, VertexFormat::*};

pub fn component_type_to_index_format(ty: gltf::accessor::DataType) -> wgpu::IndexFormat {
    match ty {
        DataType::U16 => wgpu::IndexFormat::Uint16,
        DataType::U32 => wgpu::IndexFormat::Uint32,
        _ => panic!("Unsupported index format!"),
    }
}

pub fn size_of_component_type(ty: gltf::accessor::DataType) -> usize {
    match ty {
        DataType::I8 => size_of::<u8>(),
        DataType::U8 => size_of::<i8>(),
        DataType::I16 => size_of::<i16>(),
        DataType::U16 => size_of::<u16>(),
        DataType::U32 => size_of::<u32>(),
        DataType::F32 => size_of::<f32>(),
    }
}

pub fn component_count_of_type(dims: gltf::accessor::Dimensions) -> usize {
    match dims {
        Scalar => 1,
        Vec2 => 2,
        Vec3 => 3,
        Vec4 => 4,
        Mat2 => 4,
        Mat3 => 12,
        Mat4 => 16,
    }
}

pub fn stride_of_component_type(accessor: &gltf::accessor::Accessor) -> usize {
    size_of_component_type(accessor.data_type()) * component_count_of_type(accessor.dimensions())
}

pub fn accessor_type_to_format(accessor: &gltf::accessor::Accessor) -> wgpu::VertexFormat {
    use gltf::accessor::DataType::*;
    let normalized = accessor.normalized();
    let dims = accessor.dimensions();
    let ty = accessor.data_type();
    match (normalized, dims, ty) {
        (false, Vec2, U8) => Uint8x2,
        (false, Vec4, U8) => Uint8x4,
        (false, Vec2, I8) => Sint8x2,
        (false, Vec4, I8) => Sint8x4,
        (true, Vec2, U8) => Unorm8x2,
        (true, Vec4, U8) => Unorm8x4,
        (true, Vec2, I8) => Snorm8x2,
        (true, Vec4, I8) => Snorm8x4,
        (false, Vec2, U16) => Uint16x2,
        (false, Vec4, U16) => Uint16x4,
        (false, Vec2, I16) => Sint16x2,
        (false, Vec4, I16) => Sint16x4,
        (true, Vec2, U16) => Unorm16x2,
        (true, Vec4, U16) => Unorm16x4,
        (true, Vec2, I16) => Snorm16x2,
        (true, Vec4, I16) => Snorm16x4,
        (_, Scalar, F32) => Float32,
        (_, Vec2, F32) => Float32x2,
        (_, Vec3, F32) => Float32x3,
        (_, Vec4, F32) => Float32x4,
        (_, Scalar, U32) => Uint32,
        (_, Vec2, U32) => Uint32x2,
        (_, Vec3, U32) => Uint32x3,
        (_, Vec4, U32) => Uint32x4,
        _ => panic!("Unsupported vertex format!"),
    }
}

pub fn convert_sampler(device: &wgpu::Device, sampler: gltf::texture::Sampler) -> wgpu::Sampler {
    let mag_filter = match sampler.mag_filter() {
        Some(MagFilter::Linear) => FilterMode::Nearest,
        Some(MagFilter::Nearest) => FilterMode::Linear,
        None => FilterMode::Linear,
    };
    let min_filter = match sampler.min_filter() {
        Some(
            MinFilter::Linear | MinFilter::LinearMipmapLinear | MinFilter::LinearMipmapNearest,
        ) => FilterMode::Linear,
        _ => FilterMode::Nearest,
    };
    let mipmap_filter = match sampler.min_filter() {
        Some(MinFilter::LinearMipmapLinear | MinFilter::NearestMipmapLinear) => FilterMode::Linear,
        _ => FilterMode::Nearest,
    };

    device.create_sampler(&wgpu::SamplerDescriptor {
        label: sampler.name().or(Some("Unnamed Gltf Sampler")),
        address_mode_u: wrappping_to_address_mode(sampler.wrap_s()),
        address_mode_v: wrappping_to_address_mode(sampler.wrap_t()),
        address_mode_w: wgpu::AddressMode::Repeat,
        mag_filter,
        min_filter,
        mipmap_filter,
        ..Default::default()
    })
}

pub fn convert_image_format(format: gltf::image::Format) -> TextureFormat {
    // TODO: Don't know fitting format for RGB variants
    match format {
        gltf::image::Format::R8 => TextureFormat::R8Unorm,
        gltf::image::Format::R8G8 => TextureFormat::Rg8Unorm,
        gltf::image::Format::R8G8B8 => TextureFormat::Rgba8Unorm,
        gltf::image::Format::R8G8B8A8 => TextureFormat::Rgba8Unorm,
        gltf::image::Format::R16 => TextureFormat::R16Unorm,
        gltf::image::Format::R16G16 => TextureFormat::Rg16Unorm,
        gltf::image::Format::R16G16B16 => TextureFormat::Rgba16Unorm,
        gltf::image::Format::R16G16B16A16 => TextureFormat::Rgba16Unorm,
        gltf::image::Format::R32G32B32FLOAT | gltf::image::Format::R32G32B32A32FLOAT => {
            TextureFormat::Rgba32Float
        }
    }
}

pub fn mesh_mode_to_topology(mode: gltf::mesh::Mode) -> wgpu::PrimitiveTopology {
    use gltf::mesh::Mode;
    use PrimitiveTopology::*;
    match mode {
        Mode::Triangles => TriangleList,
        Mode::TriangleStrip | Mode::TriangleFan => TriangleStrip,
        Mode::Lines => LineList,
        Mode::LineStrip => LineStrip,
        Mode::Points => PointList,
        Mode::LineLoop => todo!("Line Loop!"),
    }
}

pub fn wrappping_to_address_mode(mode: gltf::texture::WrappingMode) -> wgpu::AddressMode {
    use gltf::texture::WrappingMode;
    use wgpu::AddressMode::*;
    match mode {
        WrappingMode::MirroredRepeat => MirrorRepeat,
        WrappingMode::ClampToEdge => ClampToEdge,
        WrappingMode::Repeat => Repeat,
    }
}

// pub fn convert_dynamic_image(
//     image: image::DynamicImage,
//     srgb: bool,
// ) -> (Vec<u8>, wgpu::TextureFormat) {
//     use wgpu::TextureFormat::*;
//     match image {
//         image::DynamicImage::ImageLuma8(i) => (i.into_raw(), R8Unorm),
//         image::DynamicImage::ImageLumaA8(i) => (
//             ConvertBuffer::<ImageBuffer<Luma<u8>, Vec<u8>>>::convert(&i).into_raw(),
//             R8Unorm,
//         ),
//         image::DynamicImage::ImageRgb8(i) => (
//             ConvertBuffer::<ImageBuffer<Rgba<u8>, Vec<u8>>>::convert(&i).into_raw(),
//             if srgb { Rgba8UnormSrgb } else { Rgba8Unorm },
//         ),
//         image::DynamicImage::ImageRgba8(i) => {
//             (i.into_raw(), if srgb { Rgba8UnormSrgb } else { Rgba8Unorm })
//         }
//         image::DynamicImage::ImageBgr8(i) => (
//             ConvertBuffer::<ImageBuffer<Bgra<u8>, Vec<u8>>>::convert(&i).into_raw(),
//             if srgb { Bgra8UnormSrgb } else { Bgra8Unorm },
//         ),
//         image::DynamicImage::ImageBgra8(i) => {
//             (i.into_raw(), if srgb { Bgra8UnormSrgb } else { Bgra8Unorm })
//         }
//         i => (
//             i.into_rgba8().into_raw(),
//             if srgb { Rgba8UnormSrgb } else { Rgba8Unorm },
//         ),
//     }
// }
