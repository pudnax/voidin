use std::{
    iter::{self, Repeat},
    mem::size_of,
    num::NonZeroU64,
    ops::Range,
    time::Duration,
};

use either::Either;
use glam::Vec4;
use wgpu::util::DeviceExt;
use wgpu_profiler::GpuTimerScopeResult;

pub trait NonZeroSized: Sized {
    const NSIZE: NonZeroU64 = unsafe { NonZeroU64::new_unchecked(size_of::<Self>() as _) };
}
impl<T> NonZeroSized for T where T: Sized {}

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
