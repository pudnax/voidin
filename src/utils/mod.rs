use std::{
    io,
    iter::{self, Repeat},
    num::NonZeroU64,
    ops::Range,
    path::Path,
    time::Duration,
};

mod buffer;
mod import_resolver;
mod resource;
pub use buffer::{ResizableBuffer, ResizableBufferExt};
pub use import_resolver::ImportResolver;
pub use resource::*;

use either::Either;
use glam::Vec4;
use wgpu::util::{align_to, DeviceExt};
use wgpu_profiler::GpuTimerScopeResult;

use crate::SHADER_FOLDER;

pub trait NonZeroSized: Sized {
    const NSIZE: NonZeroU64 = {
        if std::mem::size_of::<Self>() == 0 {
            panic!("type is zero-sized");
        }
        unsafe { NonZeroU64::new_unchecked(std::mem::size_of::<Self>() as _) }
    };
}
impl<T> NonZeroSized for T where T: Sized {}

#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable, Clone, Copy, Debug)]
pub struct DrawIndexedIndirect {
    pub vertex_count: u32,
    pub instance_count: u32,
    pub base_index: u32,
    pub vertex_offset: i32,
    pub base_instance: u32,
}

pub trait Lerp: Sized {
    fn lerp(self, range: Range<Self>) -> Self;
}

impl Lerp for f32 {
    fn lerp(self, Range { start: a, end: b }: Range<Self>) -> Self {
        a * (1. - self) + b * self
    }
}

impl Lerp for f64 {
    fn lerp(self, Range { start: a, end: b }: Range<Self>) -> Self {
        a * (1. - self) + b * self
    }
}

pub trait UnwrapRepeat<T: Default + Clone, I>
where
    I: Iterator<Item = T>,
{
    fn unwrap_repeat(self) -> Either<I, Repeat<T>>;
}

impl<T: Default + Clone, I> UnwrapRepeat<T, I> for Option<I>
where
    I: Iterator<Item = T>,
{
    fn unwrap_repeat(self) -> Either<I, Repeat<T>> {
        match self {
            Some(iter) => Either::Left(iter),
            None => Either::Right(iter::repeat(T::default())),
        }
    }
}

pub fn scopes_to_console_recursive(results: &[GpuTimerScopeResult], indentation: usize) {
    for scope in results {
        if indentation > 0 {
            print!("{:<width$}", "|", width = 4 * indentation);
        }
        let time = Duration::from_micros(((scope.time.end - scope.time.start) * 1e6) as u64);
        println!("{time:?} - {}", scope.label);
        if !scope.nested_scopes.is_empty() {
            scopes_to_console_recursive(&scope.nested_scopes, indentation + 1);
        }
    }
}

/// Creates WGPU texture with color in range [0., 1.]
pub fn create_solid_color_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    color: Vec4,
) -> wgpu::Texture {
    let color = color * 255.;

    device.create_texture_with_data(
        queue,
        &wgpu::TextureDescriptor {
            label: Some(&format!("Solid Texture {:?}", color.to_array())),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            mip_level_count: 1,
            sample_count: 1,
            view_formats: &[],
        },
        bytemuck::bytes_of(&color),
    )
}

pub trait DeviceShaderExt {
    fn create_shader_with_compiler(
        &self,
        path: impl AsRef<Path>,
    ) -> color_eyre::Result<wgpu::ShaderModule>;
}

impl DeviceShaderExt for wgpu::Device {
    fn create_shader_with_compiler(
        &self,
        path: impl AsRef<Path>,
    ) -> color_eyre::Result<wgpu::ShaderModule> {
        let mut resolver = ImportResolver::new(&[SHADER_FOLDER]);
        let source = resolver.populate(&path)?.contents;

        let module = self.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: path.as_ref().to_str(),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        Ok(module)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ImageDimentions {
    pub width: u32,
    pub height: u32,
    pub unpadded_bytes_per_row: u32,
    pub padded_bytes_per_row: u32,
}

impl ImageDimentions {
    pub fn new(width: u32, height: u32, align: u32) -> Self {
        let width = align_to(width, 2);
        let height = align_to(height, 2);
        let bytes_per_pixel = std::mem::size_of::<[u8; 4]>() as u32;
        let unpadded_bytes_per_row = width * bytes_per_pixel;
        let row_padding = (align - unpadded_bytes_per_row % align) % align;
        let padded_bytes_per_row = unpadded_bytes_per_row + row_padding;
        Self {
            width,
            height,
            unpadded_bytes_per_row,
            padded_bytes_per_row,
        }
    }

    pub fn linear_size(&self) -> u64 {
        self.padded_bytes_per_row as u64 * self.height as u64
    }
}

pub fn create_folder(name: impl AsRef<Path>) -> io::Result<()> {
    match std::fs::create_dir(name) {
        Ok(_) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e),
    }

    Ok(())
}

pub trait FormatConversions {
    fn swap_srgb_suffix(self) -> wgpu::TextureFormat;
}

impl FormatConversions for wgpu::TextureFormat {
    fn swap_srgb_suffix(self) -> wgpu::TextureFormat {
        use wgpu::TextureFormat::*;
        match self {
            Rgba8UnormSrgb => Rgba8Unorm,
            Bgra8UnormSrgb => Bgra8Unorm,
            Bc1RgbaUnormSrgb => Bc1RgbaUnorm,
            Bc2RgbaUnormSrgb => Bc2RgbaUnorm,
            Bc3RgbaUnormSrgb => Bc3RgbaUnorm,
            Bc7RgbaUnormSrgb => Bc7RgbaUnorm,
            Etc2Rgb8UnormSrgb => Etc2Rgb8Unorm,
            Etc2Rgb8A1UnormSrgb => Etc2Rgb8A1Unorm,
            Etc2Rgba8UnormSrgb => Etc2Rgba8Unorm,
            Astc {
                block,
                channel: wgpu::AstcChannel::UnormSrgb,
            } => Astc {
                block,
                channel: wgpu::AstcChannel::Unorm,
            },

            Rgba8Unorm => Rgba8UnormSrgb,
            Bgra8Unorm => Bgra8UnormSrgb,
            Bc1RgbaUnorm => Bc1RgbaUnormSrgb,
            Bc2RgbaUnorm => Bc2RgbaUnormSrgb,
            Bc3RgbaUnorm => Bc3RgbaUnormSrgb,
            Bc7RgbaUnorm => Bc7RgbaUnormSrgb,
            Etc2Rgb8Unorm => Etc2Rgb8UnormSrgb,
            Etc2Rgb8A1Unorm => Etc2Rgb8A1UnormSrgb,
            Etc2Rgba8Unorm => Etc2Rgba8UnormSrgb,
            Astc {
                block,
                channel: wgpu::AstcChannel::Unorm,
            } => Astc {
                block,
                channel: wgpu::AstcChannel::UnormSrgb,
            },
            _ => self,
        }
    }
}
