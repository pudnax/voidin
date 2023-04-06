use std::{
    cell::RefCell, collections::HashMap, fmt::Display, iter::zip, num::NonZeroU32, path::Path,
};

use color_eyre::{eyre::ContextCompat, Result};
use glam::vec4;
use indexmap::IndexMap;
use pollster::FutureExt;
use wgpu::{util::DeviceExt, FilterMode};
use wgpu_profiler::{scope::Scope, GpuProfiler};
use winit::{dpi::PhysicalSize, window::Window};

use crate::{
    bind_group_layout::{self, WrappedBindGroupLayout},
    camera::CameraBinding,
    gltf::{convert_sampler, mesh_mode_to_topology, GltfDocument},
    pipeline::{self, Arena},
    utils::{self, create_solid_color_texture, NonZeroSized, UnwrapRepeat},
    view_target,
    watcher::{SpirvBytes, Watcher},
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

mod blitter;
mod global_ubo;
mod state;
use blitter::Blitter;
pub use state::AppState;

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MeshVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tex_coord: [f32; 2],
}

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub struct PipelineArgs {
    pub topology: wgpu::PrimitiveTopology,
    pub target_format: wgpu::TextureFormat,
    pub cull_mode: Option<wgpu::Face>,
    pub blend: Option<wgpu::BlendState>,
}

impl PipelineArgs {
    pub fn new(
        topology: wgpu::PrimitiveTopology,
        target_format: wgpu::TextureFormat,
        double_sided: bool,
        alpha_mode: gltf::material::AlphaMode,
    ) -> Self {
        let cull_mode = (!double_sided).then_some(wgpu::Face::Back);
        let blend = (alpha_mode == gltf::material::AlphaMode::Blend).then_some(wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
        });
        Self {
            topology,
            target_format,
            cull_mode,
            blend,
        }
    }

    fn to_desc(&self, app: &App) -> pipeline::RenderPipelineDescriptor {
        let layout = vec![
            app.global_uniform_binding.layout.clone(),
            app.camera_binding.bind_group_layout.clone(),
            app.node_bind_group_layout.clone(),
            app.material_bind_group_layout.clone(),
        ];
        let attributes = [
            wgpu::VertexFormat::Float32x3,
            wgpu::VertexFormat::Float32x3,
            wgpu::VertexFormat::Float32x2,
        ];
        let fragment_entry_point = if self.cull_mode.is_some() {
            "fs_main"
        } else {
            "fs_main_cutoff"
        };
        pipeline::RenderPipelineDescriptor {
            label: Some(self.to_string().into()),
            fragment: Some(pipeline::FragmentState {
                entry_point: fragment_entry_point.into(),
                targets: vec![Some(wgpu::ColorTargetState {
                    format: self.target_format,
                    blend: self.blend,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: self.topology,
                cull_mode: self.cull_mode,
                ..Default::default()
            },
            vertex: pipeline::VertexState {
                buffers: vec![crate::pipeline::VertexBufferLayout::from_vertex_formats(
                    wgpu::VertexStepMode::Vertex,
                    attributes,
                )],
                ..Default::default()
            },
            layout,
            ..Default::default()
        }
    }
}

impl Display for PipelineArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{{ {:?}, {:?}, {:?}, {:?} }}",
            self.topology, self.target_format, self.cull_mode, self.blend
        )
    }
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
pub struct Primitive {
    pub buffer: wgpu::Buffer,
    pub instances: Vec<wgpu::BindGroup>,
    pub draw_mode: DrawMode,
}

#[derive(Debug)]
pub struct GpuPipeline {
    pub pipeline: pipeline::RenderHandle,
    pub primitives: HashMap<Option<usize>, Vec<Primitive>>,
}

pub struct App {
    adapter: wgpu::Adapter,
    pub instance: wgpu::Instance,
    pub device: wgpu::Device,
    pub surface: wgpu::Surface,
    pub surface_config: wgpu::SurfaceConfiguration,
    depth_texture: wgpu::TextureView,
    view_target: view_target::ViewTarget,
    queue: wgpu::Queue,

    pub limits: wgpu::Limits,
    pub features: wgpu::Features,

    global_uniform_binding: global_ubo::GlobalUniformBinding,
    global_uniform: global_ubo::Uniform,

    camera_binding: CameraBinding,

    node_bind_group_layout: bind_group_layout::BindGroupLayout,
    material_bind_group_layout: bind_group_layout::BindGroupLayout,

    primitive_data: IndexMap<pipeline::RenderHandle, HashMap<Option<usize>, Vec<Primitive>>>,
    material_data: HashMap<Option<usize>, wgpu::BindGroup>,

    postprocess_pipeline: pipeline::RenderHandle,

    default_sampler: wgpu::Sampler,
    opaque_white_texture: wgpu::Texture,

    blitter: Blitter,

    profiler: RefCell<wgpu_profiler::GpuProfiler>,

    pipeline_arena: Arena,
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
        features.remove(wgpu::Features::MAPPABLE_PRIMARY_BUFFERS); // |= wgpu::Features::MAPPABLE_PRIMARY_BUFFERS;

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

        let view_target = view_target::ViewTarget::new(&device, width, height);

        let mut pipeline_arena = Arena::new(file_watcher);

        let camera_binding = CameraBinding::new(&device);
        let global_uniform_binding = global_ubo::GlobalUniformBinding::new(&device);
        let global_uniform = global_ubo::Uniform {
            time: 0.,
            frame: 0,
            resolution: [surface_config.width as f32, surface_config.height as f32],
        };

        let opaque_white_texture =
            create_solid_color_texture(&device, &queue, vec4(1., 1., 1., 1.));
        let default_sampler = device.create_sampler(&DEFAULT_SAMPLER_DESC);

        let node_bind_group_layout =
            device.create_bind_group_layout_wrap(&wgpu::BindGroupLayoutDescriptor {
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

        let material_bind_group_layout =
            device.create_bind_group_layout_wrap(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Material Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: Some(glam::Vec4::NSIZE),
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let path = "shaders/postprocess.wgsl";
        let postprocess_bind_group_layout =
            device.create_bind_group_layout_wrap(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Post Process Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT
                            | wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT
                            | wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let postprocess_desc = pipeline::RenderPipelineDescriptor {
            label: Some("Post Process Pipeline".into()),
            layout: vec![
                global_uniform_binding.layout.clone(),
                postprocess_bind_group_layout,
            ],
            depth_stencil: None,
            ..Default::default()
        };
        let postprocess_pipeline =
            pipeline_arena.process_render_pipeline_from_path(&device, path, postprocess_desc)?;

        Ok(Self {
            profiler: RefCell::new(GpuProfiler::new(4, queue.get_timestamp_period(), features)),
            blitter: Blitter::new(&device),

            adapter,
            instance,
            device,
            surface,
            surface_config,
            depth_texture,
            view_target,
            queue,

            default_sampler,
            opaque_white_texture,

            global_uniform_binding,
            global_uniform,

            camera_binding,

            limits,
            features,

            node_bind_group_layout,
            material_bind_group_layout,

            postprocess_pipeline,

            primitive_data: IndexMap::new(),
            material_data: HashMap::new(),

            pipeline_arena,
        })
    }

    pub fn render(&self, _state: &AppState) -> Result<(), wgpu::SurfaceError> {
        let mut profiler = self.profiler.borrow_mut();
        let target = self.surface.get_current_texture()?;
        let target_view = target.texture.create_view(&Default::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Main Render Encoder"),
            });

        profiler.begin_scope("Main Render Scope ", &mut encoder, &self.device);

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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

        pass.set_bind_group(0, &self.global_uniform_binding.binding, &[]);
        pass.set_bind_group(1, &self.camera_binding.binding, &[]);

        for (&pipeline, primitives) in &self.primitive_data {
            let name = self.pipeline_arena.get_descriptor(pipeline).name();
            let mut pass = Scope::start(name, &mut profiler, &mut pass, &self.device);
            pass.set_pipeline(self.pipeline_arena.get_pipeline(pipeline));

            for (material, primitives) in primitives {
                pass.set_bind_group(3, &self.material_data[material], &[]);
                for primitive in primitives {
                    pass.set_vertex_buffer(0, primitive.buffer.slice(..));

                    match &primitive.draw_mode {
                        DrawMode::Normal(draw_count) => {
                            // let mut pass = pass.scope("Draw", &self.device);
                            for bind_group in &primitive.instances {
                                pass.set_bind_group(2, bind_group, &[]);
                                pass.draw(0..*draw_count, 0..1);
                            }
                        }
                        DrawMode::Indexed { buffer, draw_count } => {
                            // let mut pass = pass.scope("Draw Indexed", &self.device);
                            pass.set_index_buffer(buffer.slice(..), wgpu::IndexFormat::Uint32);
                            for bind_group in &primitive.instances {
                                pass.set_bind_group(2, bind_group, &[]);
                                pass.draw_indexed(0..*draw_count, 0, 0..1);
                            }
                        }
                    }
                }
            }
        }

        drop(pass);

        {
            let post_process_target = self.view_target.post_process_write();
            let tex_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Post: Texture Bind Group"),
                layout: &self
                    .pipeline_arena
                    .get_descriptor(self.postprocess_pipeline)
                    .layout[1],
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&post_process_target.source),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.default_sampler),
                    },
                ],
            });
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Post Process Pass"),
                color_attachments: &[Some(post_process_target.get_color_attachment(
                    wgpu::Color {
                        r: 0.,
                        g: 0.,
                        b: 0.,
                        a: 0.0,
                    },
                ))],
                depth_stencil_attachment: None,
            });
            pass.set_bind_group(0, &self.global_uniform_binding.binding, &[]);
            pass.set_bind_group(1, &tex_bind_group, &[]);
            pass.set_pipeline(self.pipeline_arena.get_pipeline(self.postprocess_pipeline));
            pass.draw(0..3, 0..1);
        }

        self.blitter.blit_to_texture(
            &self.device,
            &mut encoder,
            &self.view_target.main_view(),
            &target_view,
            self.surface_config.format,
        );

        profiler.end_scope(&mut encoder);
        profiler.resolve_queries(&mut encoder);

        self.queue.submit(Some(encoder.finish()));
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
        self.surface.configure(&self.device, &self.surface_config);
        self.depth_texture = Self::create_depth_texture(&self.device, &self.surface_config);
        self.view_target = view_target::ViewTarget::new(&self.device, width, height);
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
            utils::scopes_to_console_recursive(&last_profile, 0);
            println!();
        }
    }

    pub fn add_gltf_model(&mut self, gltf: GltfDocument) -> Result<()> {
        for material in gltf.document.materials() {
            let pbr = material.pbr_metallic_roughness();
            let mut color = pbr.base_color_factor();
            color[3] = material.alpha_cutoff().unwrap_or(0.5);

            let buffer = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("Material Color: {:?}", material.index())),
                    contents: bytemuck::bytes_of(&color),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
            let bind_group = match pbr.base_color_texture().map(|t| t.texture()) {
                Some(tex) => {
                    let sampler = convert_sampler(&self.device, tex.sampler());
                    let image = &gltf.images[tex.source().index()];
                    let (width, height) = (image.width, image.height);
                    let image = crate::gltf::convert_to_rgba(image)?;
                    let mip_level_count = width.max(height).ilog2() + 1;
                    let size = wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    };

                    let desc = wgpu::TextureDescriptor {
                        label: None,
                        size,
                        mip_level_count,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING
                            | wgpu::TextureUsages::COPY_DST
                            | wgpu::TextureUsages::RENDER_ATTACHMENT,

                        view_formats: &[],
                    };
                    let texture = self.device.create_texture(&desc);
                    self.queue.write_texture(
                        wgpu::ImageCopyTextureBase {
                            texture: &texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        image.as_raw(),
                        wgpu::ImageDataLayout {
                            offset: 0,
                            bytes_per_row: NonZeroU32::new(width * 4),
                            rows_per_image: None,
                        },
                        size,
                    );
                    let texture_view = texture.create_view(&Default::default());

                    let mut encoder = self.device.create_command_encoder(&Default::default());
                    self.blitter
                        .generate_mipmaps(&self.device, &mut encoder, &texture);
                    self.queue.submit(Some(encoder.finish()));

                    self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: None,
                        layout: &self.material_bind_group_layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: buffer.as_entire_binding(),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                // TODO: Stick to always sRGB format... or not
                                resource: wgpu::BindingResource::TextureView(&texture_view),
                            },
                            wgpu::BindGroupEntry {
                                binding: 2,
                                resource: wgpu::BindingResource::Sampler(&sampler),
                            },
                        ],
                    })
                }
                None => {
                    let texture_view = self.opaque_white_texture.create_view(&Default::default());
                    self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: None,
                        layout: &self.material_bind_group_layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: buffer.as_entire_binding(),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::TextureView(&texture_view),
                            },
                            wgpu::BindGroupEntry {
                                binding: 2,
                                resource: wgpu::BindingResource::Sampler(&self.default_sampler),
                            },
                        ],
                    })
                }
            };

            self.material_data
                .entry(material.index())
                .or_insert(bind_group);
        }

        let mut primitive_instances: HashMap<_, Vec<_>> = HashMap::new();
        for node in gltf.document.nodes() {
            let Some(mesh) = node.mesh() else { continue; };
            let name = node.name().unwrap_or("<Unnamed>");
            let node_buffer = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("Node Buffer: {:?}", name)),
                    contents: bytemuck::bytes_of(&node.transform().matrix()),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });

            for primitive in mesh.primitives() {
                let pindex = primitive.index();
                let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(&format!("Node Bind Group: {:?} {}", name, pindex)),
                    layout: &self.node_bind_group_layout,
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

        for mesh in gltf.document.meshes() {
            let mesh_name = mesh.name().unwrap_or("<Unnamed>");
            for primitive in mesh.primitives() {
                let reader = primitive.reader(|buffer| Some(&gltf.buffers[buffer.index()]));

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
                let buffer = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some(&format!("Mesh Buffer: {mesh_name}")),
                        contents: bytemuck::cast_slice(&vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    });

                let material = primitive.material();

                let args = PipelineArgs::new(
                    mesh_mode_to_topology(primitive.mode()),
                    self.view_target.format(),
                    material.double_sided(),
                    material.alpha_mode(),
                );

                let draw_mode = match reader.read_indices() {
                    None => DrawMode::Normal(vertices.len() as _),
                    Some(indices) => {
                        let data: Vec<_> = indices.into_u32().collect();
                        let buffer =
                            self.device
                                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
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

                let path = Path::new(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/shaders/draw_mesh.wgsl"
                ));
                let pipeline = self.pipeline_arena.process_render_pipeline_from_path(
                    &self.device,
                    path,
                    args.to_desc(self),
                )?;
                let primitives = self.primitive_data.entry(pipeline).or_default();
                primitives
                    .entry(material.index())
                    .or_default()
                    .push(gpu_primitive);
            }
        }

        // Hack transparency ordering
        self.primitive_data.sort_by(|&l, _, &r, _| {
            use std::cmp::Ordering::*;
            let l_desc = self.pipeline_arena.get_descriptor(l);
            let r_desc = self.pipeline_arena.get_descriptor(r);
            match (l_desc.primitive.cull_mode, r_desc.primitive.cull_mode) {
                (None, Some(_)) => Less,
                (Some(_), None) => Greater,
                _ => Equal,
            }
        });

        Ok(())
    }

    pub fn handle_events(&mut self, path: std::path::PathBuf, module: SpirvBytes) {
        let module = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: path.to_str(),
                source: wgpu::ShaderSource::SpirV(module.into()),
            });
        self.pipeline_arena
            .reload_pipelines(&self.device, &path, &module);
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
