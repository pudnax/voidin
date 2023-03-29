use std::{cell::RefCell, collections::HashMap, fmt::Display, iter::zip};

use color_eyre::{eyre::ContextCompat, Result};
use pollster::FutureExt;
use wgpu::{util::DeviceExt, FilterMode};
use wgpu_profiler::{scope::Scope, GpuProfiler};
use winit::{dpi::PhysicalSize, window::Window};

use crate::{
    camera::CameraBinding,
    gltf::{mesh_mode_to_topology, GltfModel},
    utils::{NonZeroSized, UnwrapRepeat},
};

mod global_ubo;
mod state;
pub use state::AppState;

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct MeshVertex {
    position: [f32; 3],
    normal: [f32; 3],
    tex_coord: [f32; 2],
}

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
struct PipelineArgs {
    topology: wgpu::PrimitiveTopology,
    target_format: wgpu::TextureFormat,
}

impl Display for PipelineArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{ {:?}, {:?} }}", self.topology, self.target_format)
    }
}

fn create_mesh_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    args: &PipelineArgs,
) -> wgpu::RenderPipeline {
    let attributes = &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2];
    let shader_module =
        device.create_shader_module(wgpu::include_wgsl!("../shaders/draw_mesh.wgsl"));
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(&format!("Pipeline: {}", args.to_string())),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: &shader_module,
            entry_point: "vs_main",
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<MeshVertex>() as _,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes,
            }],
        },
        primitive: wgpu::PrimitiveState {
            topology: args.topology,
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader_module,
            entry_point: "fs_main",
            targets: &[Some(args.target_format.into())],
        }),
        depth_stencil: Some(wgpu::DepthStencilState {
            format: App::DEPTH_FORMAT,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    })
}

#[derive(Debug)]
pub enum DrawMode {
    Normal(u32),
    Indexed {
        buffer: wgpu::Buffer,
        draw_count: u32,
    },
}

#[derive(Debug)]
struct Primitive {
    pub buffer: wgpu::Buffer,
    pub instances: Vec<wgpu::BindGroup>,
    pub draw_mode: DrawMode,
}

#[derive(Debug)]
pub struct GpuPipeline {
    pub pipeline: wgpu::RenderPipeline,
    primitives: Vec<Primitive>,
}

pub struct App {
    adapter: wgpu::Adapter,
    pub instance: wgpu::Instance,
    pub device: wgpu::Device,
    pub surface: wgpu::Surface,
    pub surface_config: wgpu::SurfaceConfiguration,
    depth_texture: wgpu::TextureView,
    queue: wgpu::Queue,

    pub limits: wgpu::Limits,
    pub features: wgpu::Features,

    global_uniform_binding: global_ubo::GlobalUniformBinding,
    global_uniform: global_ubo::Uniform,

    camera_binding: CameraBinding,

    pipeline_data: HashMap<PipelineArgs, GpuPipeline>,

    profiler: RefCell<wgpu_profiler::GpuProfiler>,
}

impl App {
    const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
    const _DEFAULT_SAMPLER_DESC: wgpu::SamplerDescriptor<'static> = wgpu::SamplerDescriptor {
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

    pub fn new(window: &Window) -> Result<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
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
        let features = adapter.features();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Device"),
                    features,
                    limits: limits.clone(),
                },
                None,
            )
            .block_on()?;

        let PhysicalSize { width, height } = window.inner_size();
        let surface_config = surface
            .get_default_config(&adapter, width, height)
            .context("Surface in not supported")?;
        surface.configure(&device, &surface_config);
        let depth_texture = Self::create_depth_texture(&device, &surface_config);

        let camera_binding = CameraBinding::new(&device);
        let global_uniform_binding = global_ubo::GlobalUniformBinding::new(&device);
        let global_uniform = global_ubo::Uniform {
            time: 0.,
            frame: 0,
            resolution: [surface_config.width as f32, surface_config.height as f32],
        };

        let scene = GltfModel::import(
            "assets/sponza-optimized/Sponza.gltf",
            // "assets/glTF-Sample-Models/2.0/AntiqueCamera/glTF/AntiqueCamera.gltf",
            // "assets/glTF-Sample-Models/2.0/Buggy/glTF-Binary/Buggy.glb",
            // "assets/glTF-Sample-Models/2.0/FlightHelmet/glTF/FlightHelmet.gltf",
        )?;

        let node_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Node Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: Some(glam::Mat4::NSIZE),
                    },
                    count: None,
                }],
            });

        let mut primitive_instances: HashMap<_, Vec<wgpu::BindGroup>> = HashMap::new();
        for node in scene.document.nodes() {
            let Some(mesh) = node.mesh() else { continue; };
            let name = node.name().unwrap_or("<Unnamed>");
            let node_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("Node Buffer: {:?}", name)),
                contents: bytemuck::bytes_of(&node.transform().matrix()),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

            for primitive in mesh.primitives() {
                let pindex = primitive.index();
                let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(&format!("Node Bind Group: {:?} {}", name, pindex)),
                    layout: &node_bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: node_buffer.as_entire_binding(),
                    }],
                });
                primitive_instances
                    .entry((mesh.index(), pindex))
                    .or_default()
                    .push(bind_group);
            }
        }

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(&format!("Mesh Pipeline Layout")),
            bind_group_layouts: &[
                &global_uniform_binding.layout,
                &camera_binding.bind_group_layout,
                &node_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });

        let mut pipeline_data = HashMap::new();
        for mesh in scene.document.meshes() {
            let mesh_name = mesh.name().unwrap_or("<Unnamed>");
            for primitive in mesh.primitives() {
                let reader = primitive.reader(|buffer| Some(&scene.buffers[buffer.index()]));

                let Some(positions) = reader.read_positions() else { continue; };
                let normals = reader.read_normals().unwrap_repeat();
                let tex_coords = reader.read_tex_coords(0).map(|t| t.into_f32());
                let vertices = zip(zip(positions, normals), tex_coords.unwrap_repeat())
                    .map(|((position, normal), tex_coord)| MeshVertex {
                        position,
                        normal,
                        tex_coord,
                    })
                    .collect::<Vec<_>>();
                let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("Mesh Buffer: {mesh_name}")),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });

                let args = PipelineArgs {
                    topology: mesh_mode_to_topology(primitive.mode()),
                    target_format: surface_config.format,
                };

                let draw_mode = match reader.read_indices() {
                    None => DrawMode::Normal(vertices.len() as _),
                    Some(indices) => {
                        let data: Vec<_> = indices.into_u32().collect();
                        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some(&format!("Index Buffer {mesh_name}")),
                            contents: bytemuck::cast_slice(&data),
                            usage: wgpu::BufferUsages::INDEX,
                        });
                        DrawMode::Indexed {
                            buffer,
                            draw_count: data.len() as _,
                        }
                    }
                };

                let instances = primitive_instances
                    .remove(&(mesh.index(), primitive.index()))
                    .context("Invalid GLTF: Visited same primitive twice.")?;

                let gpu_primitive = Primitive {
                    instances,
                    draw_mode,
                    buffer,
                };

                pipeline_data
                    .entry(args)
                    .or_insert_with_key(|args| {
                        let pipeline = create_mesh_pipeline(&device, &pipeline_layout, &args);
                        GpuPipeline {
                            pipeline,
                            primitives: vec![],
                        }
                    })
                    .primitives
                    .push(gpu_primitive);
            }
        }

        Ok(Self {
            profiler: RefCell::new(GpuProfiler::new(4, queue.get_timestamp_period(), features)),
            adapter,
            instance,
            device,
            surface,
            surface_config,
            // TODO: reverse Z
            depth_texture,
            queue,

            global_uniform_binding,
            global_uniform,

            camera_binding,

            limits,
            features,

            pipeline_data,
        })
    }

    pub fn render(&self, _state: &AppState) -> Result<(), wgpu::SurfaceError> {
        let mut profiler = self.profiler.borrow_mut();
        let target = self.surface.get_current_texture()?;
        let target_view = target.texture.create_view(&Default::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Command Encoder"),
            });

        profiler.begin_scope("Main Render Scope ", &mut encoder, &self.device);

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render Pass Descriptor"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.13,
                        g: 0.13,
                        b: 0.13,
                        a: 1.0,
                    }),
                    store: true,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth_texture,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: true,
                }),
                stencil_ops: None,
            }),
        });

        pass.set_bind_group(0, &self.global_uniform_binding.binding, &[]);
        pass.set_bind_group(1, &self.camera_binding.binding, &[]);

        for (args, pipeline) in &self.pipeline_data {
            let mut pass = Scope::start(&args.to_string(), &mut profiler, &mut pass, &self.device);
            pass.set_pipeline(&pipeline.pipeline);

            for primitive in &pipeline.primitives {
                pass.set_vertex_buffer(0, primitive.buffer.slice(..));

                match &primitive.draw_mode {
                    DrawMode::Normal(draw_count) => {
                        // let mut pass = pass.scope("Draw", &self.device);
                        for bind_group in &primitive.instances {
                            pass.set_bind_group(2, &bind_group, &[]);
                            pass.draw(0..*draw_count, 0..1);
                        }
                    }
                    DrawMode::Indexed { buffer, draw_count } => {
                        // let mut pass = pass.scope("Draw Indexed", &self.device);
                        pass.set_index_buffer(buffer.slice(..), wgpu::IndexFormat::Uint32);
                        for bind_group in &primitive.instances {
                            pass.set_bind_group(2, &bind_group, &[]);
                            pass.draw_indexed(0..*draw_count, 0, 0..1);
                        }
                    }
                }
            }
        }

        drop(pass);

        profiler.end_scope(&mut encoder);

        profiler.resolve_queries(&mut encoder);

        self.queue.submit(Some(encoder.finish()));
        target.present();
        profiler.end_frame().unwrap();

        Ok(())
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if self.surface_config.width == width && self.surface_config.height == height {
            return;
        }
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
        self.depth_texture = Self::create_depth_texture(&self.device, &self.surface_config);
        self.global_uniform.resolution = [width as f32, height as f32];
    }

    pub fn update(&mut self, state: &AppState) {
        self.global_uniform.frame = state.frame_count as _;
        self.global_uniform.time = state.total_time as _;
        self.global_uniform_binding
            .update(&self.queue, &self.global_uniform);
        self.camera_binding.update(&self.queue, &state.camera);

        if state.frame_count % 100 == 0 {
            let mut last_profile = vec![];
            while let Some(profiling_data) = self.profiler.borrow_mut().process_finished_frame() {
                last_profile = profiling_data;
            }
            crate::utils::scopes_to_console_recursive(&last_profile, 0);
            println!();
        }
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
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        let tex = device.create_texture(&desc);
        tex.create_view(&Default::default())
    }

    pub fn get_info(&self) -> RendererInfo {
        let info = self.adapter.get_info();
        RendererInfo {
            device_name: info.name,
            device_type: self.get_device_type().to_string(),
            vendor_name: self.get_vendor_name().to_string(),
            backend: self.get_backend().to_string(),
        }
    }

    fn get_vendor_name(&self) -> &str {
        match self.adapter.get_info().vendor {
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
        match self.adapter.get_info().backend {
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
        match self.adapter.get_info().device_type {
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
