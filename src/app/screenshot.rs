use std::num::NonZeroU32;

use wgpu::MapMode;

use crate::Gpu;

use super::blitter::Blitter;

#[derive(Debug, Clone, Copy)]
pub struct ImageDimentions {
    pub width: u32,
    pub height: u32,
    pub unpadded_bytes_per_row: u32,
    pub padded_bytes_per_row: u32,
}

impl ImageDimentions {
    pub fn new(width: u32, height: u32, align: u32) -> Self {
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

pub struct ScreenshotCtx {
    pub image_dimentions: ImageDimentions,
    data: wgpu::Buffer,
    texture: wgpu::Texture,
}

impl ScreenshotCtx {
    pub fn new(gpu: &Gpu, width: u32, height: u32) -> Self {
        let image_dimentions =
            ImageDimentions::new(width, height, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);

        let texture = gpu.device().create_texture(&wgpu::TextureDescriptor {
            label: Some("Screen Mapped Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            mip_level_count: 1,
            sample_count: 1,
            view_formats: &[],
        });

        let data = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Screen Mapped Buffer"),
            size: image_dimentions.linear_size(),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        Self {
            image_dimentions,
            data,
            texture,
        }
    }

    pub fn resize(&mut self, gpu: &Gpu, width: u32, height: u32) {
        let new_dims = ImageDimentions::new(width, height, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        if new_dims.linear_size() > self.image_dimentions.linear_size() {
            let image_dimentions =
                ImageDimentions::new(width, height, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);

            self.data = gpu.device().create_buffer(&wgpu::BufferDescriptor {
                label: Some("Screen mapped Buffer"),
                size: image_dimentions.linear_size(),
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            self.texture = gpu.device().create_texture(&wgpu::TextureDescriptor {
                label: Some("Screen Mapped Texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                mip_level_count: 1,
                sample_count: 1,
                view_formats: &[],
            });
        }
        self.image_dimentions = new_dims;
    }

    pub fn capture_frame(
        &self,
        gpu: &Gpu,
        blitter: &Blitter,
        src_texture: &wgpu::TextureView,
        callback: impl FnOnce(Vec<u8>, ImageDimentions) + Send + 'static,
    ) {
        let view = self.texture.create_view(&Default::default());
        let mut encoder = gpu
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Screenshot"),
            });
        blitter.blit_to_texture(
            &mut encoder,
            gpu.device(),
            src_texture,
            &view,
            self.texture.format(),
        );

        let copy_size = wgpu::Extent3d {
            width: self.image_dimentions.width,
            height: self.image_dimentions.height,
            depth_or_array_layers: 1,
        };
        encoder.copy_texture_to_buffer(
            self.texture.as_image_copy(),
            wgpu::ImageCopyBuffer {
                buffer: &self.data,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: NonZeroU32::new(self.image_dimentions.padded_bytes_per_row),
                    rows_per_image: None,
                },
            },
            copy_size,
        );

        let submit = gpu.queue().submit(Some(encoder.finish()));

        let image_slice = self.data.slice(0..self.image_dimentions.linear_size());
        image_slice.map_async(MapMode::Read, |res| {
            if let Err(err) = res {
                log::error!("Oh no, failed to map buffer: {err}");
            }
        });

        gpu.device()
            .poll(wgpu::Maintain::WaitForSubmissionIndex(submit));

        let mapped_slice = image_slice.get_mapped_range();
        let frame = mapped_slice.to_vec();

        drop(mapped_slice);
        self.data.unmap();

        callback(frame, self.image_dimentions)
    }
}
