use std::{cell::RefCell, fmt::Display, path::Path, sync::Arc};

use color_eyre::{eyre::ContextCompat, Result};
use glam::{vec3, vec4, Mat4, Vec2, Vec3};

use pollster::FutureExt;
use rand::Rng;
use wgpu::FilterMode;
use wgpu_profiler::GpuProfiler;
use winit::{dpi::PhysicalSize, window::Window};

use crate::{
    camera::CameraUniformBinding,
    models::{self, GltfDocument},
    pass::{self, Pass},
    recorder::Recorder,
    utils::{
        self, create_solid_color_texture,
        world::{Read, World, Write},
        DrawIndexedIndirect, ImageDimentions, ResizableBuffer, ResizableBufferExt,
    },
    watcher::Watcher,
    Gpu, FIXED_TIME_STEP,
};

pub mod bind_group_layout;
pub mod blitter;
pub mod global_ubo;
pub mod instance;
pub mod material;
pub mod mesh;
pub mod pipeline;
mod screenshot;
pub mod state;
pub mod texture;
mod view_target;

pub(crate) use view_target::ViewTarget;

use self::{
    instance::{InstanceId, InstancePool},
    material::{MaterialId, MaterialPool},
    mesh::{BoundingSphere, MeshId, MeshPool},
    pipeline::PipelineArena,
    screenshot::ScreenshotCtx,
    state::{AppState, StateAction},
    texture::TexturePool,
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

    global_uniform: global_ubo::Uniform,

    pub world: World,

    draw_cmd_buffer: ResizableBuffer<DrawIndexedIndirect>,
    draw_cmd_bind_group: wgpu::BindGroup,

    moving_instances: ResizableBuffer<InstanceId>,
    moving_instances_bind_group: wgpu::BindGroup,

    geometry_pass: pass::geometry::Geometry,
    emit_draws_pass: pass::geometry::EmitDraws,

    postprocess_pass: pass::postprocess::PostProcess,

    update_pass: pass::compute_update::ComputeUpdate,

    default_sampler: wgpu::Sampler,

    pub blitter: blitter::Blitter,

    recorder: Recorder,
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

        let mut world = World::new(gpu.clone());
        world.insert(PipelineArena::new(gpu.clone(), file_watcher));

        let global_uniform = global_ubo::Uniform {
            time: 0.,
            frame: 0,
            dt: FIXED_TIME_STEP as _,
            custom: 0.,
            resolution: [surface_config.width as f32, surface_config.height as f32],
        };

        create_solid_color_texture(gpu.device(), gpu.queue(), vec4(1., 1., 1., 1.));
        let default_sampler = gpu.device().create_sampler(&DEFAULT_SAMPLER_DESC);

        let draw_cmd_buffer = ResizableBuffer::new(
            gpu.device(),
            wgpu::BufferUsages::INDIRECT | wgpu::BufferUsages::STORAGE,
        );
        let draw_cmd_bind_group = draw_cmd_buffer.create_storage_write_bind_group(&mut world);

        let path = Path::new("shaders").join("postprocess.wgsl");
        let postprocess_pass = pass::postprocess::PostProcess::new(&mut world, path)?;

        let geometry_pass = pass::geometry::Geometry::new(&mut world)?;
        let emit_draws_pass = pass::geometry::EmitDraws::new(&mut world)?;

        let profiler = RefCell::new(GpuProfiler::new(
            4,
            gpu.queue().get_timestamp_period(),
            features,
        ));

        let moving_instances = gpu
            .device()
            .create_resizable_buffer(wgpu::BufferUsages::STORAGE);
        let moving_instances_bind_group =
            moving_instances.create_storage_read_bind_group(&mut world);

        let path = Path::new("shaders").join("compute_update.wgsl");
        let update_pass = pass::compute_update::ComputeUpdate::new(&world, path)?;

        Ok(Self {
            surface,
            surface_config,
            depth_texture,
            view_target,

            default_sampler,

            global_uniform,

            postprocess_pass,

            world,

            draw_cmd_buffer,
            draw_cmd_bind_group,

            moving_instances,
            moving_instances_bind_group,

            geometry_pass,
            emit_draws_pass,

            update_pass,

            profiler,
            blitter: blitter::Blitter::new(gpu.device()),
            screenshot_ctx: ScreenshotCtx::new(&gpu, width, height),
            recorder: Recorder::new(),

            gpu,
        })
    }

    pub fn setup_scene(&mut self) -> Result<()> {
        let now = std::time::Instant::now();
        let mut instances = vec![];

        let gltf_scene = GltfDocument::import(
            self,
            "assets/glTF-Sample-Models/2.0/Sponza/glTF/Sponza.gltf",
            // "assets/glTF-Sample-Models/2.0/AntiqueCamera/glTF/AntiqueCamera.gltf",
            // "assets/glTF-Sample-Models/2.0/Buggy/glTF-Binary/Buggy.glb",
            // "assets/glTF-Sample-Models/2.0/FlightHelmet/glTF/FlightHelmet.gltf",
            // "assets/glTF-Sample-Models/2.0/DamagedHelmet/glTF-Binary/DamagedHelmet.glb",
        )?;

        for scene in gltf_scene.document.scenes() {
            instances.extend(gltf_scene.scene_data(
                scene,
                Mat4::from_rotation_y(std::f32::consts::PI / 2.)
                    * Mat4::from_translation(vec3(7., -5., 1.))
                    * Mat4::from_scale(Vec3::splat(3.)),
            ));
        }

        let helmet = GltfDocument::import(
            self,
            "assets/glTF-Sample-Models/2.0/DamagedHelmet/glTF-Binary/DamagedHelmet.glb",
        )?;
        for scene in helmet.document.scenes() {
            instances.extend(helmet.scene_data(
                scene,
                Mat4::from_translation(vec3(0., 0., 9.)) * Mat4::from_scale(Vec3::splat(3.)),
            ));
        }

        let gltf_ferris = GltfDocument::import(self, "assets/ferris3d_v1.0.glb")?;
        for scene in gltf_ferris.document.scenes() {
            instances.extend(gltf_ferris.scene_data(
                scene,
                Mat4::from_translation(vec3(-3., -5.0, -4.)) * Mat4::from_scale(Vec3::splat(3.)),
            ));
        }
        for scene in gltf_ferris.document.scenes() {
            instances.extend(gltf_ferris.scene_data(
                scene,
                Mat4::from_translation(vec3(2., -5.0, -2.)) * Mat4::from_scale(Vec3::splat(3.)),
            ));
        }

        let sphere_mesh = models::make_uv_sphere(&mut self.get_mesh_pool_mut(), 0.9, 10);

        let mut instance_pool = self.world.get_mut::<InstancePool>()?;
        instance_pool.add(&instances);

        let mut moving_instances = vec![];
        let mut rng = rand::thread_rng();
        let num = 10;
        for i in 0..num {
            let r = 3.5;
            let angle = std::f32::consts::TAU * (i as f32) / num as f32;
            let x = r * angle.cos();
            let y = r * angle.sin();

            moving_instances.push(instance::Instance {
                transform: Mat4::from_translation(vec3(x, y, -17.)),
                mesh: sphere_mesh,
                material: MaterialId::new(
                    rng.gen_range(0..self.get_material_pool().buffer.len() as u32),
                ),
                ..Default::default()
            });

            for scene in gltf_ferris.document.scenes() {
                moving_instances.extend(gltf_ferris.scene_data(
                    scene,
                    Mat4::from_translation(vec3(x, y + 0., -9.))
                        * Mat4::from_rotation_z(angle)
                        * Mat4::from_scale(Vec3::splat(2.5)),
                ));
            }
        }

        let moving_instances_id = instance_pool.add(&moving_instances);
        self.moving_instances.push(&self.gpu, &moving_instances_id);

        let mut encoder = self.device().create_command_encoder(&Default::default());
        self.draw_cmd_buffer
            .set_len(&self.gpu.device, &mut encoder, instance_pool.count() as _);
        drop(instance_pool);

        self.draw_cmd_bind_group = self
            .draw_cmd_buffer
            .create_storage_write_bind_group(&mut self.world);
        self.moving_instances_bind_group = self
            .moving_instances
            .create_storage_read_bind_group(&mut self.world);

        println!("Scene complete: {:?}", now.elapsed());

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
            draw_cmd_bind_group: &self.draw_cmd_bind_group,
            draw_cmd_buffer: &self.draw_cmd_buffer,
        };
        self.emit_draws_pass
            .record(&self.world, &mut encoder, &self.view_target, emit_resource);

        let geometry_resource = pass::geometry::GeometryResource {
            depth_texture: &self.depth_texture,
            draw_cmd_buffer: &self.draw_cmd_buffer,
        };
        self.geometry_pass.record(
            &self.world,
            &mut encoder,
            &self.view_target,
            geometry_resource,
        );

        let resource = pass::postprocess::PostProcessResource {
            sampler: &self.default_sampler,
        };
        self.postprocess_pass
            .record(&self.world, &mut encoder, &self.view_target, resource);

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

        if self.recorder.is_active() {
            self.capture_frame(|frame, _| {
                self.recorder.record(frame);
            });
        }

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

        if self.recorder.is_active() {
            self.recorder.finish();
        }
    }

    pub fn update(&mut self, state: &AppState, actions: Vec<StateAction>) -> Result<()> {
        self.global_uniform.frame = state.frame_count as _;
        self.global_uniform.time = state.total_time as _;
        self.world
            .get_mut::<global_ubo::GlobalUniformBinding>()?
            .update(self.gpu.queue(), &self.global_uniform);
        self.world
            .get_mut::<CameraUniformBinding>()?
            .update(self.gpu.queue(), &state.camera);

        let mut encoder =
            self.gpu
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Compute Update"),
                });
        let resources = pass::compute_update::ComputeUpdateResourse {
            idx_bind_group: &self.moving_instances_bind_group,
            dispatch_size: self.moving_instances.len() as u32,
        };
        self.update_pass
            .record(&self.world, &mut encoder, &self.view_target, resources);
        self.gpu.queue().submit(Some(encoder.finish()));

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
                StateAction::Screenshot => {
                    self.capture_frame(|frame, dims| {
                        self.recorder.screenshot(frame, dims);
                    });
                }
                StateAction::StartRecording => {
                    self.recorder.start(self.screenshot_ctx.image_dimentions)
                }
                StateAction::FinishRecording => self.recorder.finish(),
            }
        }
        Ok(())
    }

    pub fn handle_events(&mut self, path: std::path::PathBuf, source: String) {
        let device = self.gpu.device();
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = self
            .device()
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: path.to_str(),
                source: wgpu::ShaderSource::Wgsl(source.into()),
            });
        match device.pop_error_scope().block_on() {
            None => {}
            Some(err) => {
                log::error!("Validation error on shader compilation.");
                eprintln!("{err}");
                return;
            }
        }
        self.world
            .get_mut::<PipelineArena>()
            .unwrap()
            .reload_pipelines(&path, &module);
    }

    pub fn capture_frame(&self, callback: impl FnOnce(Vec<u8>, ImageDimentions)) {
        let (frame, dims) = self.screenshot_ctx.capture_frame(
            &self.gpu,
            &self.blitter,
            self.view_target.main_view(),
        );
        callback(frame, dims)
    }

    pub fn add_mesh(
        &mut self,
        vertices: &[Vec3],
        normals: &[Vec3],
        tex_coords: &[Vec2],
        indices: &[u32],
        bounding_sphere: BoundingSphere,
    ) -> MeshId {
        self.world.get_mut::<MeshPool>().unwrap().add(
            vertices,
            normals,
            tex_coords,
            indices,
            bounding_sphere,
        )
    }

    pub fn get_material_pool(&self) -> Read<MaterialPool> {
        self.world.get::<MaterialPool>().unwrap()
    }

    pub fn get_material_pool_mut(&self) -> Write<MaterialPool> {
        self.world.get_mut::<MaterialPool>().unwrap()
    }

    pub fn get_texture_pool(&self) -> Read<TexturePool> {
        self.world.get::<TexturePool>().unwrap()
    }

    pub fn get_texture_pool_mut(&self) -> Write<TexturePool> {
        self.world.get_mut::<TexturePool>().unwrap()
    }

    pub fn get_mesh_pool(&self) -> Read<MeshPool> {
        self.world.get::<MeshPool>().unwrap()
    }

    pub fn get_mesh_pool_mut(&self) -> Write<MeshPool> {
        self.world.get_mut::<MeshPool>().unwrap()
    }

    pub fn get_instance_pool(&self) -> Read<InstancePool> {
        self.world.get::<InstancePool>().unwrap()
    }

    pub fn get_instance_pool_mut(&self) -> Write<InstancePool> {
        self.world.get_mut::<InstancePool>().unwrap()
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
