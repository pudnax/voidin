use core::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use glam::{Vec2, Vec3};

use crate::{
    utils::{NonZeroSized, ResizableBuffer, ResizableBufferExt, Resource},
    Gpu,
};

use super::bind_group_layout::{self, WrappedBindGroupLayout};

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct MeshId(u32);

impl From<MeshId> for u32 {
    fn from(value: MeshId) -> u32 {
        value.0
    }
}
impl From<MeshId> for usize {
    fn from(value: MeshId) -> usize {
        value.0 as _
    }
}

impl MeshId {
    pub fn id(&self) -> u32 {
        self.0
    }
}

// TODO: rearrange
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct MeshInfo {
    vertex_offset: i32,
    base_index: u32,
    index_count: u32,
}

pub struct MeshManager {
    vertex_offset: AtomicU32,
    base_index: AtomicU32,
    mesh_index: AtomicU32,

    pub mesh_info_layout: bind_group_layout::BindGroupLayout,
    pub mesh_info_bind_group: wgpu::BindGroup,
    pub mesh_info: ResizableBuffer<MeshInfo>,
    pub mesh_cpu: Vec<MeshInfo>,

    pub vertices: ResizableBuffer<Vec3>,
    pub normals: ResizableBuffer<Vec3>,
    pub tex_coords: ResizableBuffer<Vec2>,
    pub indices: ResizableBuffer<u32>,

    gpu: Arc<Gpu>,
}

impl Resource for MeshManager {
    fn init(gpu: Arc<Gpu>) -> Self {
        Self::new(gpu)
    }
}

impl MeshManager {
    pub fn new(gpu: Arc<Gpu>) -> Self {
        let mesh_info = gpu
            .device()
            .create_resizable_buffer(wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC);
        let vertices = gpu
            .device()
            .create_resizable_buffer(wgpu::BufferUsages::VERTEX);
        let normals = gpu
            .device()
            .create_resizable_buffer(wgpu::BufferUsages::VERTEX);
        let tex_coords = gpu
            .device()
            .create_resizable_buffer(wgpu::BufferUsages::VERTEX);
        let indices = gpu
            .device()
            .create_resizable_buffer(wgpu::BufferUsages::INDEX);

        let mesh_info_layout =
            gpu.device()
                .create_bind_group_layout_wrap(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Mesh Info Bind Group Layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE
                            | wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: Some(MeshInfo::NSIZE),
                        },
                        count: None,
                    }],
                });
        let mesh_info_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Mesh Info Bind Group"),
            layout: &mesh_info_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: mesh_info.as_entire_binding(),
            }],
        });

        Self {
            vertex_offset: AtomicU32::new(0),
            base_index: AtomicU32::new(0),
            mesh_index: AtomicU32::new(0),

            mesh_info_layout,
            mesh_info_bind_group,
            mesh_info,
            mesh_cpu: vec![],

            vertices,
            normals,
            tex_coords,
            indices,

            gpu,
        }
    }

    pub fn count(&self) -> u32 {
        self.mesh_index.load(Ordering::Relaxed)
    }

    pub fn add(
        &mut self,
        vertices: &[Vec3],
        normals: &[Vec3],
        tex_coords: &[Vec2],
        indices: &[u32],
    ) -> MeshId {
        let vertex_count = vertices.len() as u32;
        let vertex_offset = self
            .vertex_offset
            .fetch_add(vertex_count, Ordering::Relaxed);

        self.vertices.push(&self.gpu, vertices);
        self.normals.push(&self.gpu, normals);
        self.tex_coords.push(&self.gpu, tex_coords);

        let index_count = indices.len() as u32;
        let base_index = self.base_index.fetch_add(index_count, Ordering::Relaxed);

        self.indices.push(&self.gpu, indices);
        let mesh_index = self.mesh_index.fetch_add(1, Ordering::Relaxed);

        let mesh_info = MeshInfo {
            vertex_offset: vertex_offset as i32,
            base_index,
            index_count,
        };
        self.mesh_cpu.push(mesh_info);
        let was_resized = self.mesh_info.push(&self.gpu, &[mesh_info]);
        if was_resized {
            let desc = wgpu::BindGroupDescriptor {
                label: Some("Mesh Info Bind Group"),
                layout: &self.mesh_info_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.mesh_info.as_entire_binding(),
                }],
            };
            self.mesh_info_bind_group = self.gpu.device().create_bind_group(&desc);
        }

        log::info!("Added new mesh with id: {mesh_index}");
        MeshId(mesh_index)
    }
}
