use std::sync::atomic::{AtomicU8, Ordering};

use wgpu::{Color, RenderPassColorAttachment, Texture, TextureFormat, TextureView};

pub struct PostProcessWrite<'a> {
    pub source: &'a TextureView,
    pub destination: &'a TextureView,
}

impl<'a> PostProcessWrite<'a> {
    pub fn get_color_attachment(&self, color: wgpu::Color) -> RenderPassColorAttachment {
        RenderPassColorAttachment {
            view: &self.destination,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(color),
                store: true,
            },
        }
    }
}

pub struct ViewTarget {
    a: Texture,
    b: Texture,
    aview: TextureView,
    bview: TextureView,
    main_texture: AtomicU8,
}

impl ViewTarget {
    pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let mut desc = wgpu::TextureDescriptor {
            label: Some("Target Texture A"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[Self::FORMAT, Self::FORMAT.add_srgb_suffix()],
        };
        let a = device.create_texture(&desc);
        desc.label = Some("Target Texture B");
        let b = device.create_texture(&desc);
        Self {
            main_texture: AtomicU8::new(0),
            aview: a.create_view(&Default::default()),
            bview: b.create_view(&Default::default()),
            a,
            b,
        }
    }

    pub fn get_color_attachment(&self, color: Color) -> RenderPassColorAttachment {
        RenderPassColorAttachment {
            view: self.main_view(),
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(color),
                store: true,
            },
        }
    }

    pub fn format(&self) -> TextureFormat {
        Self::FORMAT
    }

    pub fn main_texture(&self) -> &Texture {
        if self.main_texture.load(Ordering::Relaxed) == 0 {
            &self.a
        } else {
            &self.b
        }
    }

    pub fn main_texture_other(&self) -> &Texture {
        if self.main_texture.load(Ordering::Relaxed) == 0 {
            &self.b
        } else {
            &self.a
        }
    }

    pub fn main_view(&self) -> &TextureView {
        if self.main_texture.load(Ordering::Relaxed) == 0 {
            &self.aview
        } else {
            &self.bview
        }
    }

    pub fn main_view_other(&self) -> &TextureView {
        if self.main_texture.load(Ordering::Relaxed) == 0 {
            &self.bview
        } else {
            &self.aview
        }
    }

    pub fn post_process_write(&self) -> PostProcessWrite {
        let old_target = self.main_texture.fetch_xor(1, Ordering::Relaxed);
        if old_target == 0 {
            PostProcessWrite {
                source: &self.aview,
                destination: &self.bview,
            }
        } else {
            PostProcessWrite {
                source: &self.bview,
                destination: &self.aview,
            }
        }
    }
}
