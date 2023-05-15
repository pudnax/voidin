use std::{
    path::Path,
    sync::atomic::{AtomicU8, Ordering},
};

use crate::{
    app::{
        bind_group_layout::{BindGroupLayout, WrappedBindGroupLayout},
        gbuffer::GBuffer,
        pipeline::{ComputeHandle, ComputePipelineDescriptor, PipelineArena},
        ViewTarget, DEFAULT_SAMPLER_DESC,
    },
    camera::CameraUniformBinding,
    utils::world::World,
};
use color_eyre::Result;
use wgpu::{util::align_to, CommandEncoder};

use super::Pass;

struct CombinedTexture {
    texture: wgpu::TextureView,
    sample_bind_group: wgpu::BindGroup,
    storage_bind_group: wgpu::BindGroup,
}

impl CombinedTexture {
    pub fn new(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        read_bgl: &wgpu::BindGroupLayout,
        write_bgl: &wgpu::BindGroupLayout,
        label: Option<&str>,
    ) -> Self {
        let texture = device
            .create_texture(&wgpu::TextureDescriptor {
                label,
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default());
        let sample_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Read Texture BG"),
            layout: read_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&texture),
            }],
        });
        let storage_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Write Texture BG"),
            layout: write_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&texture),
            }],
        });

        Self {
            texture,
            sample_bind_group,
            storage_bind_group,
        }
    }
}

pub struct Taa {
    read_texture_layout: BindGroupLayout,
    write_texture_layout: BindGroupLayout,

    active_texture: AtomicU8,
    history: [CombinedTexture; 2],
    motion_texture: CombinedTexture,

    reprojection_pipeline: ComputeHandle,
    taa_pipeline: ComputeHandle,
    sampler: wgpu::BindGroup,
}

impl Taa {
    pub fn new(world: &World, gbuffer: &GBuffer, width: u32, height: u32) -> Result<Self> {
        let device = world.gpu.device();
        let mut pipeline_arena = world.get_mut::<PipelineArena>()?;
        let camera_binding = world.get::<CameraUniformBinding>()?;
        let sampler_layout =
            device.create_bind_group_layout_wrap(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Sampler BGL"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                }],
            });
        let read_texture_layout =
            device.create_bind_group_layout_wrap(&wgpu::BindGroupLayoutDescriptor {
                label: Some("History Texture BGL"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                }],
            });
        let write_texture_layout =
            device.create_bind_group_layout_wrap(&wgpu::BindGroupLayoutDescriptor {
                label: Some("History Texture BGL"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                }],
            });

        let history_textures = std::array::from_fn(|i| {
            CombinedTexture::new(
                device,
                width,
                height,
                wgpu::TextureFormat::Rgba16Float,
                &read_texture_layout,
                &write_texture_layout,
                Some(&format!("History Texture {i}")),
            )
        });

        let motion_texture = CombinedTexture::new(
            device,
            width,
            height,
            wgpu::TextureFormat::Rgba16Float,
            &read_texture_layout,
            &write_texture_layout,
            Some(&format!("Motion Texture")),
        );

        let pipeline_desc = ComputePipelineDescriptor {
            label: Some("Reprojection Pipeline".into()),
            layout: vec![
                camera_binding.bind_group_layout.clone(),
                gbuffer.bind_group_layout.clone(),
                write_texture_layout.clone(),
            ],
            ..Default::default()
        };
        let shader_path = Path::new("shaders").join("reproject.wgsl");
        let reprojection_pipeline =
            pipeline_arena.process_compute_pipeline_from_path(shader_path, pipeline_desc)?;

        let pipeline_desc = ComputePipelineDescriptor {
            label: Some("Taa Pipeline".into()),
            layout: vec![
                sampler_layout.clone(),
                // Input Texture
                read_texture_layout.clone(),
                // History Texture
                read_texture_layout.clone(),
                // Motion Texture
                read_texture_layout.clone(),
                // Output Texture
                write_texture_layout.clone(),
            ],
            ..Default::default()
        };
        let shader_path = Path::new("shaders").join("taa.wgsl");
        let taa_pipeline =
            pipeline_arena.process_compute_pipeline_from_path(shader_path, pipeline_desc)?;

        let sampler = device.create_sampler(&DEFAULT_SAMPLER_DESC);
        let sampler = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Sampler BG"),
            layout: &sampler_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Sampler(&sampler),
            }],
        });

        Ok(Self {
            read_texture_layout,
            write_texture_layout,

            active_texture: AtomicU8::new(0),
            history: history_textures,
            motion_texture,

            reprojection_pipeline,
            taa_pipeline,
            sampler,
        })
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.history = std::array::from_fn(|i| {
            CombinedTexture::new(
                device,
                width,
                height,
                wgpu::TextureFormat::Rgba16Float,
                &self.read_texture_layout,
                &self.write_texture_layout,
                Some(&format!("History Texture {i}")),
            )
        });

        self.motion_texture = CombinedTexture::new(
            device,
            width,
            height,
            wgpu::TextureFormat::Rgba16Float,
            &self.read_texture_layout,
            &self.write_texture_layout,
            Some(&format!("Motion Texture")),
        );
    }

    pub fn output_texture(&self) -> &wgpu::TextureView {
        &self.history[self.active_texture.load(Ordering::Relaxed) as usize].texture
    }
}

pub struct TaaResource<'a> {
    pub view_target: &'a ViewTarget,
    pub gbuffer: &'a GBuffer,
    pub width_height: (u32, u32),
}

impl Pass for Taa {
    type Resources<'a> = TaaResource<'a>;

    fn record(&self, world: &World, encoder: &mut CommandEncoder, resource: Self::Resources<'_>) {
        let input_history = self.active_texture.fetch_xor(1, Ordering::Relaxed) as usize;
        let output_history = input_history ^ 1;

        let camera = world.unwrap::<CameraUniformBinding>();
        let arena = world.unwrap::<PipelineArena>();

        let input_bind_group = world
            .device()
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Post: Texture Bind Group"),
                layout: &self.read_texture_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(resource.view_target.main_view()),
                }],
            });

        let (width, height) = resource.width_height;
        let x = align_to(width, 8) / 8;
        let y = align_to(height, 8) / 8;

        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Reprojection Pass"),
        });

        cpass.set_pipeline(arena.get_pipeline(self.reprojection_pipeline));
        cpass.set_bind_group(0, &camera.binding, &[]);
        cpass.set_bind_group(1, &resource.gbuffer.bind_group, &[]);
        cpass.set_bind_group(2, &self.motion_texture.storage_bind_group, &[]);
        cpass.dispatch_workgroups(x, y, 1);
        drop(cpass);

        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Taa Pass"),
        });

        cpass.set_pipeline(arena.get_pipeline(self.taa_pipeline));
        cpass.set_bind_group(0, &self.sampler, &[]);
        cpass.set_bind_group(1, &input_bind_group, &[]);
        cpass.set_bind_group(2, &self.history[input_history].sample_bind_group, &[]);
        cpass.set_bind_group(3, &self.motion_texture.sample_bind_group, &[]);
        cpass.set_bind_group(4, &self.history[output_history].storage_bind_group, &[]);
        cpass.dispatch_workgroups(x, y, 1);
        drop(cpass);
    }
}
