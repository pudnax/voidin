use std::{cell::RefCell, fmt::Display, path::Path, sync::Arc};

use color_eyre::{eyre::ContextCompat, Result};
use glam::{vec3, vec4, Mat4, Vec2, Vec3};

use pollster::FutureExt;
use wgpu::FilterMode;
use wgpu_profiler::GpuProfiler;
use winit::{dpi::PhysicalSize, window::Window};

use crate::{
    camera::CameraUniformBinding,
    models::{self, GltfDocument},
    pass::{self, Pass},
    utils::{
        self, create_solid_color_texture, save_screenshot, DrawIndexedIndirect, NonZeroSized,
        ResizableBuffer,
    },
    watcher::{SpirvBytes, Watcher},
    Gpu,
};

pub mod bind_group_layout;
pub mod blitter;
mod global_ubo;
pub mod instance;
pub mod material;
pub mod mesh;
pub mod pipeline;
mod screenshot;
pub mod state;
pub mod texture;
mod view_target;

pub use screenshot::ImageDimentions;
pub(crate) use view_target::ViewTarget;

use self::{
    bind_group_layout::WrappedBindGroupLayout,
    instance::InstancesManager,
    material::{Material, MaterialId, MaterialManager},
    mesh::{MeshId, MeshManager},
    pipeline::Handle,
    screenshot::ScreenshotCtx,
    state::{AppState, StateAction},
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

    geometry_pass: pass::geometry::Geometry,
    emit_draws_pass: pass::geometry::EmitDraws,

    postprocess_pipeline: pass::postprocess::PostProcess,

    default_sampler: wgpu::Sampler,

    pub blitter: blitter::Blitter,

    pipeline_arena: pipeline::Arena,

    screenshot_ctx: ScreenshotCtx,
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

        let path = Path::new("shaders").join("postprocess.wgsl");
        let postprocess_pipeline = pass::postprocess::PostProcess::new(
            &mut pipeline_arena,
            global_uniform_binding.layout.clone(),
            path,
        )?;

        let geometry_pass = pass::geometry::Geometry::new(
            &mut pipeline_arena,
            global_uniform_binding.layout.clone(),
            camera_uniform.bind_group_layout.clone(),
            texture_manager.bind_group_layout.clone(),
            mesh_manager.mesh_info_layout.clone(),
            instance_manager.bind_group_layout.clone(),
            material_manager.bind_group_layout.clone(),
        )?;
        let emit_draws_pass = pass::geometry::EmitDraws::new(
            &mut pipeline_arena,
            mesh_manager.mesh_info_layout.clone(),
            instance_manager.bind_group_layout.clone(),
            draw_cmd_layout.clone(),
        )?;

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

            geometry_pass,
            emit_draws_pass,

            profiler,
            blitter: blitter::Blitter::new(gpu.device()),
            screenshot_ctx: ScreenshotCtx::new(&gpu, width, height),

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
            "assets/glTF-Sample-Models/2.0/Sponza/glTF/Sponza.gltf",
            // "assets/sponza-optimized/Sponza.gltf",
            // "assets/glTF-Sample-Models/2.0/AntiqueCamera/glTF/AntiqueCamera.gltf",
            // "assets/glTF-Sample-Models/2.0/Buggy/glTF-Binary/Buggy.glb",
            // "assets/glTF-Sample-Models/2.0/FlightHelmet/glTF/FlightHelmet.gltf",
            // "assets/glTF-Sample-Models/2.0/DamagedHelmet/glTF-Binary/DamagedHelmet.glb",
        )?;

        for scene in gltf_scene.document.scenes() {
            let instances = gltf_scene.scene_data(
                scene,
                Mat4::from_rotation_y(std::f32::consts::PI / 2.)
                    * Mat4::from_translation(vec3(0., -4., 0.))
                    * Mat4::from_scale(Vec3::splat(3.)),
            );
            dbg!(instances.len());
            self.instance_manager.add(&instances)
        }

        let sphere_mesh = models::sphere_mesh(self, 0.6, 20, 20);

        let mut instances = vec![];
        let num = 10;
        for i in 0..num {
            let r = 4.0;
            let x = r * (std::f32::consts::TAU * (i as f32) / num as f32).cos();
            let y = r * (std::f32::consts::TAU * (i as f32) / num as f32).sin();

            instances.push(instance::Instance {
                transform: Mat4::from_translation(vec3(x, y, 2.)),
                mesh: sphere_mesh,
                material: MaterialId::default(),
                ..Default::default()
            });
        }
        // self.instance_manager.add(&instances);

        // let mut instances = vec![];
        let ferris = models::ObjModel::import(self, "assets/ferris3d_v1.0.obj")?;
        for (mesh, material) in &ferris {
            instances.push(instance::Instance::new(
                Mat4::from_translation(vec3(-3., -4.1, -8.)) * Mat4::from_scale(Vec3::splat(3.)),
                *mesh,
                *material,
            ));
        }
        self.instance_manager.add(&instances);
        // let mut instances = vec![];
        // for (mesh, material) in &ferris {
        //     instances.push(instance::Instance::new(
        //         Mat4::from_translation(vec3(-3., -4.1, -5.)) * Mat4::from_scale(Vec3::splat(3.)),
        //         *mesh,
        //         *material,
        //     ));
        // }
        // self.instance_manager.add(&instances);

        let ferris_gltf = GltfDocument::import(self, "assets/ferris3d_v1.0.glb")?;
        for scene in ferris_gltf.document.scenes() {
            let instances = ferris_gltf.scene_data(
                scene,
                Mat4::from_translation(vec3(3., -4.5, -5.)) * Mat4::from_scale(Vec3::splat(3.0)),
            );
            self.instance_manager.add(&instances)
        }
        dbg!(instances.len());
        dbg!(self.instance_manager.instances.len());

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

        let emit_resource = pass::geometry::EmitDrawsResource {
            arena: &self.pipeline_arena,
            mesh_info_bind_group: &self.mesh_manager.mesh_info_bind_group,
            instance_bind_group: &self.instance_manager.bind_group,
            draw_cmd_bind_group: &self.draw_cmd_bind_group,
            draw_cmd_buffer: &self.draw_cmd_buffer,
        };
        self.emit_draws_pass
            .record(&mut encoder, &self.view_target, emit_resource);

        let geometry_resource = pass::geometry::GeometryResource {
            arena: &self.pipeline_arena,
            depth_texture: &self.depth_texture,
            global_binding: &self.global_uniform_binding.binding,
            camera_binding: &self.camera_uniform.binding,
            textures_bind_group: &self.textures_bind_group,
            instance_bind_group: &self.instance_manager.bind_group,
            material_bind_group: &self.material_manager.bind_group,
            draw_cmd_buffer: &self.draw_cmd_buffer,
            mesh_manager: &self.mesh_manager,
        };
        self.geometry_pass
            .record(&mut encoder, &self.view_target, geometry_resource);

        let resource = pass::postprocess::PostProcessResource {
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

        self.screenshot_ctx.resize(&self.gpu, width, height);
    }

    pub fn update(&mut self, state: &AppState, actions: Vec<StateAction>) {
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

        for action in actions {
            match action {
                StateAction::Screenshot => self.capture_frame(|frame, dims| {
                    save_screenshot(frame, dims);
                }),
            }
        }
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

    pub fn capture_frame(&self, callback: impl FnOnce(Vec<u8>, ImageDimentions) + Send + 'static) {
        self.screenshot_ctx.capture_frame(
            &self.gpu,
            &self.blitter,
            self.view_target.main_view(),
            callback,
        )
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

    pub fn get_pipeline<H: Handle>(&self, handle: H) -> &H::Pipeline {
        self.pipeline_arena.get_pipeline(handle)
    }

    pub fn get_pipeline_desc<H: Handle>(&self, handle: H) -> &H::Descriptor {
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
