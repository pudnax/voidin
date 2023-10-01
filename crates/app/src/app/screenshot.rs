use std::sync::Arc;

use wgpu::MapMode;

use crate::Gpu;

use components::{world::World, Blitter, ImageDimentions};

pub struct ScreenshotCtx {
    pub image_dimentions: ImageDimentions,
    data: wgpu::Buffer,
    texture: wgpu::Texture,
}

impl ScreenshotCtx {
    pub fn new(gpu: &Gpu, width: u32, height: u32) -> Self {
        let image_dimentions =
            ImageDimentions::new(width, height, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);

        let data = gpu.device().create_buffer(&wgpu::BufferDescriptor {
            label: Some("Screen Copy Buffer"),
            size: image_dimentions.linear_size(),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let texture = gpu.device().create_texture(&wgpu::TextureDescriptor {
            label: Some("Screen Copy Texture"),
            size: wgpu::Extent3d {
                width: image_dimentions.width,
                height: image_dimentions.height,
                depth_or_array_layers: 1,
            },
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            mip_level_count: 1,
            sample_count: 1,
            view_formats: &[],
        });

        Self {
            image_dimentions,
            data,
            texture,
        }
    }

    pub fn resize(&mut self, gpu: &Gpu, width: u32, height: u32) {
        let new_dims = ImageDimentions::new(width, height, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let image_dimentions =
            ImageDimentions::new(width, height, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);

        self.data = gpu.device().create_buffer(&wgpu::BufferDescriptor {
            label: Some("Screen Copy Buffer"),
            size: image_dimentions.linear_size(),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        self.texture = gpu.device().create_texture(&wgpu::TextureDescriptor {
            label: Some("Screen Copy Texture"),
            size: wgpu::Extent3d {
                width: image_dimentions.width,
                height: image_dimentions.height,
                depth_or_array_layers: 1,
            },
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            mip_level_count: 1,
            sample_count: 1,
            view_formats: &[],
        });
        self.image_dimentions = new_dims;
    }

    pub fn capture_frame(
        &self,
        world: &World,
        blitter: &Blitter,
        src_texture: &wgpu::TextureView,
        callback: impl FnOnce(Arc<wgpu::Buffer>, ImageDimentions) + Send + 'static,
    ) {
        let device = world.device();
        let dims = self.image_dimentions;

        let view = self.texture.create_view(&Default::default());
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Screenshot"),
        });
        blitter.blit_to_texture(
            &mut encoder,
            world,
            src_texture,
            &view,
            self.texture.format(),
        );

        encoder.copy_texture_to_buffer(
            self.texture.as_image_copy(),
            wgpu::ImageCopyBuffer {
                buffer: &self.data,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(dims.padded_bytes_per_row),
                    rows_per_image: None,
                },
            },
            self.texture.size(),
        );

        let download = Arc::new(device.create_buffer(&wgpu::BufferDescriptor {
            size: dims.linear_size(),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
            label: Some("Download Buffer"),
        }));
        encoder.copy_buffer_to_buffer(&self.data, 0, &download, 0, dims.linear_size());
        world.queue().submit(Some(encoder.finish()));

        let buff = download.clone();
        let image_slice = download.slice(0..dims.linear_size());
        image_slice.map_async(MapMode::Read, move |res| {
            if let Err(err) = res {
                log::error!("Oh no, failed to map buffer: {err}");
                return;
            }

            callback(buff, dims);
        });
    }
}
