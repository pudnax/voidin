use std::{cell::RefCell, collections::HashMap, fmt::Display};

use color_eyre::{
    eyre::{eyre, ContextCompat},
    Result,
};
use pollster::FutureExt;
use wgpu::util::DeviceExt;
use wgpu_profiler::GpuProfiler;
use winit::{dpi::PhysicalSize, window::Window};

use crate::{
    camera::CameraBinding,
    gltf::{
        accessor_type_to_format, component_type_to_index_format, mesh_mode_to_topology,
        stride_of_component_type,
    },
    model::{self, DrawModel, Vertex},
    resources,
    utils::NonZeroSized,
};

mod global_ubo;
mod state;
pub use state::AppState;

pub struct ShaderLocation(u32);

impl ShaderLocation {
    fn new(s: gltf::Semantic) -> Option<Self> {
        Some(match s {
            gltf::Semantic::Positions => Self(0),
            gltf::Semantic::Normals => Self(1),
            _ => return None,
        })
    }
}

impl TryFrom<gltf::Semantic> for ShaderLocation {
    type Error = color_eyre::Report;
    fn try_from(v: gltf::Semantic) -> Result<Self, Self::Error> {
        Ok(match v {
            gltf::Semantic::Positions => Self(0),
            gltf::Semantic::Normals => Self(1),
            _ => return Err(eyre!("Unsupported primitive semantic")),
        })
    }
}

#[derive(Debug)]
pub enum DrawMode {
    Normal(u32),
    Indexed {
        buffer: wgpu::Buffer,
        offset: u64,
        ty: wgpu::IndexFormat,
        draw_count: u32,
    },
}

#[derive(Debug)]
pub struct GpuPrimitive {
    pub pipeline: wgpu::RenderPipeline,
    pub buffers: Vec<wgpu::Buffer>,
    pub draw_mode: DrawMode,
}

struct Instance {
    position: glam::Vec3,
    rotation: glam::Quat,
}

impl Instance {
    fn to_raw(&self) -> InstanceRaw {
        InstanceRaw {
            model: (glam::Mat4::from_translation(self.position)
                * glam::Mat4::from_quat(self.rotation))
            .to_cols_array_2d(),
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct InstanceRaw {
    #[allow(dead_code)]
    model: [[f32; 4]; 4],
}

impl InstanceRaw {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<InstanceRaw>() as wgpu::BufferAddress,
            // We need to switch from using a step mode of Vertex to Instance
            // This means that our shaders will only change to use the next
            // instance when the shader starts processing a new instance
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    // While our vertex shader only uses locations 0, and 1 now, in later tutorials we'll
                    // be using 2, 3, and 4, for Vertex. We'll start at slot 5 not conflict with them later
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // A mat4 takes up 4 vertex slots as it is technically 4 vec4s. We need to define a slot
                // for each vec4. We don't have to do this in code though.
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 12]>() as wgpu::BufferAddress,
                    shader_location: 8,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
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

    obj_model: model::Model,

    camera_binding: CameraBinding,

    render_pipeline: wgpu::RenderPipeline,

    instances: Vec<Instance>,
    instance_buffer: wgpu::Buffer,

    document: gltf::Document,
    node_data: HashMap<usize, wgpu::BindGroup>,
    primitive_data: HashMap<(usize, usize), GpuPrimitive>,

    profiler: RefCell<wgpu_profiler::GpuProfiler>,
}

impl App {
    const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

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

        const NUM_INSTANCES_PER_ROW: u32 = 10;
        const SPACE_BETWEEN: f32 = 3.0;
        let instances = (0..NUM_INSTANCES_PER_ROW)
            .flat_map(|z| {
                (0..NUM_INSTANCES_PER_ROW).map(move |x| {
                    let x = SPACE_BETWEEN * (x as f32 - NUM_INSTANCES_PER_ROW as f32 / 2.0);
                    let z = SPACE_BETWEEN * (z as f32 - NUM_INSTANCES_PER_ROW as f32 / 2.0);

                    let position = glam::vec3(x, 0.0001, z);

                    let rotation = glam::Quat::from_axis_angle(
                        position.normalize(),
                        std::f32::consts::PI / 4.,
                    );

                    Instance { position, rotation }
                })
            })
            .collect::<Vec<_>>();

        let instance_data = instances.iter().map(Instance::to_raw).collect::<Vec<_>>();
        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Instance Buffer"),
            contents: bytemuck::cast_slice(&instance_data),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("texture_bind_group_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let obj_model = resources::load_model(
            "assets/cube/cube.obj",
            &device,
            &queue,
            &texture_bind_group_layout,
        )?;

        let shader_module =
            device.create_shader_module(wgpu::include_wgsl!("../shaders/model.wgsl"));

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Pipeline Layout"),
            bind_group_layouts: &[
                &global_uniform_binding.layout,
                &camera_binding.bind_group_layout,
                &texture_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: "vs_main",
                buffers: &[model::ModelVertex::DESC, InstanceRaw::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: "fs_main",
                targets: &[Some(surface_config.format.into())],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: Self::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        let (document, buffers, _images) =
            gltf::import("assets/glTF-Sample-Models/2.0/AntiqueCamera/glTF/AntiqueCamera.gltf")?;

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

        let mut node_data = HashMap::new();
        for node in document.nodes().filter(|n| n.mesh().is_some()) {
            let name = node.name().unwrap_or("<Unnamed>");
            let node_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("Node Buffer: {:?}", name)),
                contents: bytemuck::bytes_of(&node.transform().matrix()),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
            let node_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("Node Bind Group: {:?}", name)),
                layout: &node_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: node_buffer.as_entire_binding(),
                }],
            });

            node_data.insert(node.index(), node_bind_group);
        }

        let mut primitive_data = HashMap::new();
        for mesh in document.meshes() {
            let mesh_name = mesh.name().unwrap_or("<Unnamed>");
            dbg!(mesh_name);
            for primitive in mesh.primitives() {
                struct VertexLayout {
                    array_stride: u64,
                    step_mode: wgpu::VertexStepMode,
                }
                let mut vertex_buffer_layouts = vec![];
                let mut vertex_attributes = vec![];
                let mut primitive_buffers = vec![];
                let mut draw_count = 0;
                for (semantic, accessor) in primitive.attributes() {
                    let Some(buffer_view) = accessor.view() else { continue; };
                    println!(
                        "Buffer {:?}: №{} {:?}",
                        buffer_view.name(),
                        buffer_view.buffer().index(),
                        &semantic,
                    );

                    let Some(shader_location) = ShaderLocation::new(semantic.clone()) else {
                        println!("Skip");
                        continue;
                    };

                    let array_stride = dbg!(buffer_view
                        .stride()
                        .unwrap_or(stride_of_component_type(&accessor)));
                    dbg!(buffer_view.offset());
                    dbg!(accessor.offset());
                    vertex_buffer_layouts.push(VertexLayout {
                        array_stride: array_stride as _,
                        step_mode: wgpu::VertexStepMode::Vertex,
                    });
                    vertex_attributes.push([wgpu::VertexAttribute {
                        format: accessor_type_to_format(&accessor),
                        offset: accessor.offset() as _,
                        shader_location: shader_location.0,
                    }]);

                    let buffer_view_data = &buffers[buffer_view.buffer().index()];
                    let label = format!("Vertex Buffer {:?}: {:?}", mesh.name(), semantic);
                    primitive_buffers.push(device.create_buffer_init(
                        &wgpu::util::BufferInitDescriptor {
                            label: Some(&label),
                            contents: &buffer_view_data[buffer_view.offset()..]
                                [..buffer_view.length()],
                            usage: wgpu::BufferUsages::VERTEX,
                        },
                    ));

                    draw_count = dbg!(accessor.count());
                }

                let mut vertex_buffers = vec![];
                for (layout, attr) in std::iter::zip(vertex_buffer_layouts, &vertex_attributes) {
                    vertex_buffers.push(wgpu::VertexBufferLayout {
                        array_stride: layout.array_stride,
                        step_mode: layout.step_mode,
                        attributes: attr,
                    });
                }
                let pipeline_layout =
                    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some(&format!("Render Pipeline Layout: {}", mesh_name)),
                        bind_group_layouts: &[
                            &global_uniform_binding.layout,
                            &camera_binding.bind_group_layout,
                            &node_bind_group_layout,
                        ],
                        push_constant_ranges: &[],
                    });
                let shader_module =
                    device.create_shader_module(wgpu::include_wgsl!("../shaders/draw_mesh.wgsl"));
                let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some(&format!("Render Pipeline: {}", mesh_name)),
                    layout: Some(&pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shader_module,
                        entry_point: "vs_main",
                        buffers: &vertex_buffers,
                    },
                    primitive: wgpu::PrimitiveState {
                        topology: mesh_mode_to_topology(primitive.mode()),
                        cull_mode: Some(wgpu::Face::Back),
                        ..Default::default()
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader_module,
                        entry_point: "fs_main",
                        targets: &[Some(surface_config.format.into())],
                    }),
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: Self::DEPTH_FORMAT,
                        depth_write_enabled: true,
                        depth_compare: wgpu::CompareFunction::Less,
                        stencil: wgpu::StencilState::default(),
                        bias: wgpu::DepthBiasState::default(),
                    }),
                    multisample: wgpu::MultisampleState::default(),
                    multiview: None,
                });

                let draw_mode = match primitive.indices() {
                    None => DrawMode::Normal(draw_count as _),
                    Some(acc) => {
                        let Some(buffer_view) = acc.view() else { continue; };
                        let buffer = &buffers[buffer_view.buffer().index()];
                        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some(&format!("Index Buffer {mesh_name}")),
                            contents: &buffer[buffer_view.offset()..][..buffer_view.length()],
                            usage: wgpu::BufferUsages::INDEX,
                        });
                        DrawMode::Indexed {
                            buffer,
                            offset: acc.offset() as _,
                            ty: component_type_to_index_format(acc.data_type()),
                            draw_count: acc.count() as _,
                        }
                    }
                };

                let gpu_primitive = GpuPrimitive {
                    pipeline,
                    buffers: primitive_buffers,
                    draw_mode,
                };

                primitive_data.insert((mesh.index(), primitive.index()), gpu_primitive);

                println!();
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

            render_pipeline: pipeline,

            obj_model,

            limits,
            features,

            instances,
            instance_buffer,

            document,
            node_data,
            primitive_data,
        })
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
        for (&node, gpu_node) in &self.node_data {
            pass.set_bind_group(2, &gpu_node, &[]);

            let node = self.document.nodes().nth(node).unwrap();
            let mesh = node.mesh().unwrap();
            for primitive in mesh.primitives() {
                let gpu_primitive = &self.primitive_data[&(mesh.index(), primitive.index())];

                pass.set_pipeline(&gpu_primitive.pipeline);
                for (i, buffer) in gpu_primitive.buffers.iter().enumerate() {
                    pass.set_vertex_buffer(i as _, buffer.slice(..));
                }

                match &gpu_primitive.draw_mode {
                    DrawMode::Normal(draw_count) => pass.draw(0..*draw_count, 0..1),
                    DrawMode::Indexed {
                        buffer,
                        offset,
                        ty,
                        draw_count,
                    } => {
                        pass.set_index_buffer(buffer.slice(*offset..), *ty);
                        pass.draw_indexed(0..*draw_count, 0, 0..1)
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
