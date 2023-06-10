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

        let texture = gpu.device().create_texture(&wgpu::TextureDescriptor {
            label: Some("Screen Mapped Texture"),
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

        let data = gpu.device().create_buffer(&wgpu::BufferDescriptor {
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
    ) -> (Vec<u8>, ImageDimentions) {
        let device = world.device();

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
                    bytes_per_row: Some(self.image_dimentions.padded_bytes_per_row),
                    rows_per_image: None,
                },
            },
            self.texture.size(),
        );

        let submit = world.queue().submit(Some(encoder.finish()));

        let image_slice = self.data.slice(0..self.image_dimentions.linear_size());
        image_slice.map_async(MapMode::Read, |res| {
            if let Err(err) = res {
                log::error!("Oh no, failed to map buffer: {err}");
            }
        });

        device.poll(wgpu::Maintain::WaitForSubmissionIndex(submit));

        let mapped_slice = image_slice.get_mapped_range();
        let frame = mapped_slice.to_vec();

        drop(mapped_slice);
        self.data.unmap();

        (frame, self.image_dimentions)
    }
}
