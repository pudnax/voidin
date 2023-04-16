use std::{
    fs::File,
    io::{self, BufWriter, Write},
    iter::{self, Repeat},
    num::NonZeroU64,
    ops::Range,
    path::Path,
    time::{Duration, Instant},
};

mod buffer;
pub use buffer::{ResizableBuffer, ResizableBufferExt};

use either::Either;
use glam::Vec4;
use wgpu::util::DeviceExt;
use wgpu_profiler::GpuTimerScopeResult;

use crate::{app::ImageDimentions, SCREENSHOTS_FOLDER, SHADER_FOLDER};

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

pub fn align_to(size: u32, align: u32) -> u32 {
    (size + align - 1) & !(align - 1)
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
        let parser = ocl_include::Parser::builder()
            .add_source(
                ocl_include::source::Fs::builder()
                    .include_dir(uni_path::Path::new(SHADER_FOLDER))?
                    .build(),
            )
            .build();

        let parsed_res = parser.parse(uni_path::Path::new(&path.as_ref().to_string_lossy()))?;
        let source = parsed_res.collect().0;

        let module = self.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: path.as_ref().to_str(),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        Ok(module)
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

pub fn save_screenshot(
    frame: Vec<u8>,
    image_dimentions: ImageDimentions,
) -> std::thread::JoinHandle<color_eyre::Result<()>> {
    std::thread::spawn(move || {
        let now = Instant::now();
        let screenshots_folder = Path::new(SCREENSHOTS_FOLDER);
        create_folder(screenshots_folder)?;
        let path = screenshots_folder.join(format!(
            "screenshot-{}.png",
            chrono::Local::now().format("%d-%m-%Y-%H-%M-%S")
        ));
        let file = File::create(path)?;
        let w = BufWriter::new(file);
        let mut encoder =
            png::Encoder::new(w, image_dimentions.width as _, image_dimentions.height as _);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let padded_bytes = image_dimentions.padded_bytes_per_row as _;
        let unpadded_bytes = image_dimentions.unpadded_bytes_per_row as _;
        let mut writer = encoder
            .write_header()?
            .into_stream_writer_with_size(unpadded_bytes)?;
        writer.set_filter(png::FilterType::Paeth);
        writer.set_adaptive_filter(png::AdaptiveFilterType::Adaptive);
        for chunk in frame
            .chunks(padded_bytes)
            .map(|chunk| &chunk[..unpadded_bytes])
        {
            writer.write_all(chunk)?;
        }
        writer.finish()?;
        log::info!("Encode image: {:#.2?}", now.elapsed());
        Ok(())
    })
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
