use std::{cell::RefCell, fmt::Display, sync::Arc};

use color_eyre::{eyre::ContextCompat, Result};
use glam::{vec4, Mat4, Vec2, Vec3};

use pollster::FutureExt;
use wgpu::{FilterMode, IndexFormat};
use wgpu_profiler::GpuProfiler;
use winit::{dpi::PhysicalSize, window::Window};

use crate::{
    camera::CameraUniformBinding,
    models::GltfDocument,
    utils::{
        self, align_to, create_solid_color_texture, DrawIndexedIndirect, NonZeroSized,
        ResizableBuffer,
    },
    watcher::{SpirvBytes, Watcher},
    Gpu, Pass,
};

pub mod bind_group_layout;
pub mod blitter;
mod global_ubo;
pub mod instance;
pub mod material;
pub mod mesh;
pub mod pipeline;
mod postprocess_pass;
pub mod state;
pub mod texture;
mod view_target;

pub(crate) use view_target::ViewTarget;

use self::{
    bind_group_layout::WrappedBindGroupLayout,
    instance::InstancesManager,
    material::{Material, MaterialId, MaterialManager},
    mesh::{MeshId, MeshManager},
    pipeline::{
        ComputeHandle, ComputePipelineDescriptor, Handle, RenderHandle, RenderPipelineDescriptor,
    },
    state::AppState,
    texture::{TextureId, TextureManager},
};

pub(crate) const DEFAULT_SAMPLER_DESC: wgpu::SamplerDescriptor<'static> = wgpu::SamplerDescriptor {
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

pub struct App {
    pub gpu: Arc<Gpu>,
    pub surface: wgpu::Surface,
    pub surface_config: wgpu::SurfaceConfiguration,
    depth_texture: wgpu::TextureView,
    view_target: view_target::ViewTarget,

    global_uniform_binding: global_ubo::GlobalUniformBinding,
    global_uniform: global_ubo::Uniform,

    camera_uniform: CameraUniformBinding,

    pub texture_manager: TextureManager,
    pub mesh_manager: MeshManager,
    pub material_manager: MaterialManager,
    pub instance_manager: InstancesManager,
    pub textures_bind_group: wgpu::BindGroup,
    draw_cmd_buffer: ResizableBuffer<DrawIndexedIndirect>,
    draw_cmd_bind_group: wgpu::BindGroup,
    draw_cmd_layout: bind_group_layout::BindGroupLayout,

    draw_indirect: RenderHandle,
    compute_indirect: ComputeHandle,

    postprocess_pipeline: postprocess_pass::PostProcessPipeline,

    default_sampler: wgpu::Sampler,

    pub blitter: blitter::Blitter,

    pipeline_arena: pipeline::Arena,

    profiler: RefCell<wgpu_profiler::GpuProfiler>,
}

impl App {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
    pub const SAMPLE_COUNT: u32 = 1;

    pub fn new(window: &Window, file_watcher: Watcher) -> Result<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            dx12_shader_compiler: wgpu::Dx12Compiler::Fxc,
        });

        let surface = unsafe { instance.create_surface(&window) }?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .block_on()
            .context("Failed to create Adapter")?;

        let limits = adapter.limits();
        let mut features = adapter.features();
        features.remove(wgpu::Features::MAPPABLE_PRIMARY_BUFFERS);

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Device"),
                    features,
                    limits,
                },
                None,
            )
            .block_on()?;
        let gpu = Arc::new(Gpu {
            device,
            queue,
            adapter,
        });

        let PhysicalSize { width, height } = window.inner_size();
        let surface_config = surface
            .get_default_config(gpu.adapter(), width, height)
            .context("Surface in not supported")?;
        surface.configure(gpu.device(), &surface_config);
        let depth_texture = Self::create_depth_texture(gpu.device(), &surface_config);

        let view_target = view_target::ViewTarget::new(gpu.device(), width, height);

        let mut pipeline_arena = pipeline::Arena::new(gpu.clone(), file_watcher);
        let texture_manager = TextureManager::new(gpu.device(), gpu.queue());
        let textures_bind_group = texture_manager.create_bind_group(gpu.device());
        let instance_manager = InstancesManager::new(gpu.clone());
        let mesh_manager = MeshManager::new(gpu.clone());
        let material_manager = MaterialManager::new(gpu.clone());

        let camera_uniform = CameraUniformBinding::new(gpu.device());
        let global_uniform_binding = global_ubo::GlobalUniformBinding::new(gpu.device());
        let global_uniform = global_ubo::Uniform {
            time: 0.,
            frame: 0,
            resolution: [surface_config.width as f32, surface_config.height as f32],
        };

        create_solid_color_texture(gpu.device(), gpu.queue(), vec4(1., 1., 1., 1.));
        let default_sampler = gpu.device().create_sampler(&DEFAULT_SAMPLER_DESC);

        let draw_cmd_buffer = ResizableBuffer::new(
            gpu.device(),
            wgpu::BufferUsages::INDIRECT | wgpu::BufferUsages::STORAGE,
        );
        let draw_cmd_layout =
            gpu.device()
                .create_bind_group_layout_wrap(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Draw Commands Bind Group Layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE
                            | wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: Some(utils::DrawIndexedIndirect::NSIZE),
                        },
                        count: None,
                    }],
                });
        let draw_cmd_bind_group = gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Draw Commands Bind Group"),
            layout: &draw_cmd_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: draw_cmd_buffer.as_entire_binding(),
            }],
        });

        let path = "shaders/postprocess.wgsl";
        let postprocess_pipeline = postprocess_pass::PostProcessPipeline::new(
            &mut pipeline_arena,
            global_uniform_binding.layout.clone(),
            path,
        )?;

        let path = "shaders/emit_draws.wgsl";
        let comp_desc = ComputePipelineDescriptor {
            label: Some("Compute Indirect Pipeline Layout".into()),
            layout: vec![
                mesh_manager.mesh_info_layout.clone(),
                instance_manager.bind_group_layout.clone(),
                draw_cmd_layout.clone(),
            ],
            push_constant_ranges: vec![],
            entry_point: "emit_draws".into(),
        };
        let compute_indirect =
            pipeline_arena.process_compute_pipeline_from_path(path, comp_desc)?;
        let path = "shaders/draw_indirect.wgsl";
        let render_desc = RenderPipelineDescriptor {
            label: Some("Draw Indirect Pipeline".into()),
            layout: vec![
                global_uniform_binding.layout.clone(),
                camera_uniform.bind_group_layout.clone(),
                texture_manager.bind_group_layout.clone(),
                mesh_manager.mesh_info_layout.clone(),
                instance_manager.bind_group_layout.clone(),
                material_manager.bind_group_layout.clone(),
            ],
            vertex: pipeline::VertexState {
                entry_point: "vs_main".into(),
                buffers: vec![
                    // Positions
                    pipeline::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Vec3>() as _,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: wgpu::vertex_attr_array![0 => Float32x3].to_vec(),
                    },
                    // Normals
                    pipeline::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Vec3>() as _,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: wgpu::vertex_attr_array![1 => Float32x3].to_vec(),
                    },
                    // UVs
                    pipeline::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Vec2>() as _,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: wgpu::vertex_attr_array![2 => Float32x2].to_vec(),
                    },
                ],
            },
            fragment: Some(pipeline::FragmentState {
                entry_point: "fs_main".into(),
                ..Default::default()
            }),
            ..Default::default()
        };
        let draw_indirect = pipeline_arena.process_render_pipeline_from_path(path, render_desc)?;

        let profiler = RefCell::new(GpuProfiler::new(
            4,
            gpu.queue().get_timestamp_period(),
            features,
        ));

        Ok(Self {
            texture_manager,
            mesh_manager,
            material_manager,
            instance_manager,
            textures_bind_group,

            draw_cmd_buffer,
            draw_cmd_bind_group,
            draw_cmd_layout,

            draw_indirect,
            compute_indirect,

            profiler,
            blitter: blitter::Blitter::new(gpu.device()),

            gpu,
            surface,
            surface_config,
            depth_texture,
            view_target,

            default_sampler,

            global_uniform_binding,
            global_uniform,

            camera_uniform,

            postprocess_pipeline,

            pipeline_arena,
        })
    }

    pub fn setup_scene(&mut self) -> Result<()> {
        let gltf_scene = GltfDocument::import(
            self,
            "assets/sponza-optimized/Sponza.gltf",
            // "assets/glTF-Sample-Models/2.0/AntiqueCamera/glTF/AntiqueCamera.gltf",
            // "assets/glTF-Sample-Models/2.0/Buggy/glTF-Binary/Buggy.glb",
            // "assets/glTF-Sample-Models/2.0/FlightHelmet/glTF/FlightHelmet.gltf",
            // "assets/glTF-Sample-Models/2.0/DamagedHelmet/glTF-Binary/DamagedHelmet.glb",
        )?;

        for scene in gltf_scene.document.scenes() {
            let instances = gltf_scene.scene_data(scene, Mat4::IDENTITY);
            self.instance_manager.add(&instances)
        }

        let mut encoder = self.device().create_command_encoder(&Default::default());
        self.draw_cmd_buffer.set_len(
            &self.gpu.device,
            &mut encoder,
            self.instance_manager.count() as _,
        );
        self.draw_cmd_bind_group = self.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Draw Commands Bind Group"),
            layout: &self.draw_cmd_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.draw_cmd_buffer.as_entire_binding(),
            }],
        });

        Ok(())
    }

    pub fn render(&self, _state: &AppState) -> Result<(), wgpu::SurfaceError> {
        let mut profiler = self.profiler.borrow_mut();
        let target = self.surface.get_current_texture()?;
        let target_view = target.texture.create_view(&Default::default());

        let mut encoder = self
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Main Render Encoder"),
            });

        profiler.begin_scope("Main Render Scope ", &mut encoder, self.device());

        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Emit Draws Pass"),
        });

        cpass.set_pipeline(self.get_pipeline(self.compute_indirect));
        cpass.set_bind_group(0, &self.mesh_manager.mesh_info_bind_group, &[]);
        cpass.set_bind_group(1, &self.instance_manager.bind_group, &[]);
        cpass.set_bind_group(2, &self.draw_cmd_bind_group, &[]);
        cpass.dispatch_workgroups(align_to(self.draw_cmd_buffer.len() as _, 32), 1, 1);
        drop(cpass);

        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render Pass"),
            color_attachments: &[Some(self.view_target.get_color_attachment(wgpu::Color {
                r: 0.13,
                g: 0.13,
                b: 0.13,
                a: 0.0,
            }))],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth_texture,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0.0),
                    store: true,
                }),
                stencil_ops: None,
            }),
        });

        rpass.set_pipeline(self.get_pipeline(self.draw_indirect));
        rpass.set_bind_group(0, &self.global_uniform_binding.binding, &[]);
        rpass.set_bind_group(1, &self.camera_uniform.binding, &[]);
        rpass.set_bind_group(2, &self.textures_bind_group, &[]);
        rpass.set_bind_group(3, &self.mesh_manager.mesh_info_bind_group, &[]);
        rpass.set_bind_group(4, &self.instance_manager.bind_group, &[]);
        rpass.set_bind_group(5, &self.material_manager.bind_group, &[]);

        // TODO: combine them, manager -> meshes
        rpass.set_vertex_buffer(0, self.mesh_manager.vertices.full_slice());
        rpass.set_vertex_buffer(1, self.mesh_manager.normals.full_slice());
        rpass.set_vertex_buffer(2, self.mesh_manager.tex_coords.full_slice());
        rpass.set_index_buffer(self.mesh_manager.indices.full_slice(), IndexFormat::Uint32);
        rpass.multi_draw_indexed_indirect(
            &self.draw_cmd_buffer,
            0,
            self.draw_cmd_buffer.len() as _,
        );

        drop(rpass);

        let resource = postprocess_pass::PostProcessResource {
            arena: &self.pipeline_arena,
            global_binding: &self.global_uniform_binding.binding,
            sampler: &self.default_sampler,
        };
        self.postprocess_pipeline
            .record(&mut encoder, &self.view_target, resource);

        self.blitter.blit_to_texture(
            &mut encoder,
            self.gpu.device(),
            self.view_target.main_view(),
            &target_view,
            self.surface_config.format,
        );

        profiler.end_scope(&mut encoder);
        profiler.resolve_queries(&mut encoder);

        self.gpu.queue().submit(Some(encoder.finish()));
        target.present();

        profiler.end_frame().ok();

        Ok(())
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if self.surface_config.width == width && self.surface_config.height == height {
            return;
        }
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface
            .configure(self.gpu.device(), &self.surface_config);
        self.depth_texture = Self::create_depth_texture(self.gpu.device(), &self.surface_config);
        self.view_target = view_target::ViewTarget::new(self.gpu.device(), width, height);
        self.global_uniform.resolution = [width as f32, height as f32];
    }

    pub fn update(&mut self, state: &AppState) {
        self.global_uniform.frame = state.frame_count as _;
        self.global_uniform.time = state.total_time as _;
        self.global_uniform_binding
            .update(self.gpu.queue(), &self.global_uniform);
        self.camera_uniform.update(self.gpu.queue(), &state.camera);

        if state.frame_count % 500 == 0 && std::env::var("GPU_PROFILING").is_ok() {
            let mut last_profile = vec![];
            while let Some(profiling_data) = self.profiler.borrow_mut().process_finished_frame() {
                last_profile = profiling_data;
            }
            utils::scopes_to_console_recursive(&last_profile, 0);
            println!();
        }
    }

    pub fn add_mesh(
        &mut self,
        vertices: &[Vec3],
        normals: &[Vec3],
        tex_coords: &[Vec2],
        indices: &[u32],
    ) -> MeshId {
        self.mesh_manager
            .add(vertices, normals, tex_coords, indices)
    }

    pub fn add_material(&mut self, material: Material) -> MaterialId {
        self.material_manager.add(material)
    }

    pub fn add_texture(&mut self, view: wgpu::TextureView) -> TextureId {
        self.texture_manager.add(view)
    }

    pub fn handle_events(&mut self, path: std::path::PathBuf, module: SpirvBytes) {
        let module = self
            .device()
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: path.to_str(),
                source: wgpu::ShaderSource::SpirV(module.into()),
            });
        self.pipeline_arena.reload_pipelines(&path, &module);
    }

    fn get_pipeline<H: Handle>(&self, handle: H) -> &H::Pipeline {
        self.pipeline_arena.get_pipeline(handle)
    }

    #[allow(dead_code)]
    fn get_pipeline_desc<H: Handle>(&self, handle: H) -> &H::Descriptor {
        self.pipeline_arena.get_descriptor(handle)
    }

    pub fn queue(&self) -> &wgpu::Queue {
        self.gpu.queue()
    }

    pub fn device(&self) -> &wgpu::Device {
        self.gpu.device()
    }

    fn create_depth_texture(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
    ) -> wgpu::TextureView {
        let size = wgpu::Extent3d {
            width: config.width,
            height: config.height,
            depth_or_array_layers: 1,
        };
        let desc = wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size,
            mip_level_count: 1,
            sample_count: Self::SAMPLE_COUNT,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        let tex = device.create_texture(&desc);
        tex.create_view(&Default::default())
    }

    pub fn get_info(&self) -> RendererInfo {
        let info = self.gpu.adapter().get_info();
        RendererInfo {
            device_name: info.name,
            device_type: self.get_device_type().to_string(),
            vendor_name: self.get_vendor_name().to_string(),
            backend: self.get_backend().to_string(),
        }
    }

    fn get_vendor_name(&self) -> &str {
        match self.gpu.adapter().get_info().vendor {
            0x1002 => "AMD",
            0x1010 => "ImgTec",
            0x10DE => "NVIDIA Corporation",
            0x13B5 => "ARM",
            0x5143 => "Qualcomm",
            0x8086 => "INTEL Corporation",
            _ => "Unknown vendor",
        }
    }

    fn get_backend(&self) -> &str {
        match self.gpu.adapter().get_info().backend {
            wgpu::Backend::Empty => "Empty",
            wgpu::Backend::Vulkan => "Vulkan",
            wgpu::Backend::Metal => "Metal",
            wgpu::Backend::Dx12 => "Dx12",
            wgpu::Backend::Dx11 => "Dx11",
            wgpu::Backend::Gl => "GL",
            wgpu::Backend::BrowserWebGpu => "Browser WGPU",
        }
    }

    fn get_device_type(&self) -> &str {
        match self.gpu.adapter().get_info().device_type {
            wgpu::DeviceType::Other => "Other",
            wgpu::DeviceType::IntegratedGpu => "Integrated GPU",
            wgpu::DeviceType::DiscreteGpu => "Discrete GPU",
            wgpu::DeviceType::VirtualGpu => "Virtual GPU",
            wgpu::DeviceType::Cpu => "CPU",
        }
    }
}

#[derive(Debug)]
pub struct RendererInfo {
    pub device_name: String,
    pub device_type: String,
    pub vendor_name: String,
    pub backend: String,
}

impl Display for RendererInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Vendor name: {}", self.vendor_name)?;
        writeln!(f, "Device name: {}", self.device_name)?;
        writeln!(f, "Device type: {}", self.device_type)?;
        writeln!(f, "Backend: {}", self.backend)?;
        Ok(())
    }
}
