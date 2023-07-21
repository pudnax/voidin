use std::sync::Arc;

use components::{
    bind_group_layout::{self, WrappedBindGroupLayout},
    Gpu, Instance, InstanceId, NonZeroSized, ResizableBuffer, ResizableBufferExt,
};

pub struct InstancePool {
    pub instances_data: Vec<Instance>,
    pub instances: ResizableBuffer<Instance>,

    pub bind_group: wgpu::BindGroup,
    pub bind_group_layout: bind_group_layout::BindGroupLayout,
    gpu: Arc<Gpu>,
}

impl InstancePool {
    const LAYOUT: wgpu::BindGroupLayoutDescriptor<'static> = wgpu::BindGroupLayoutDescriptor {
        label: Some("Draw Instances Bind Group Layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::COMPUTE.union(wgpu::ShaderStages::VERTEX_FRAGMENT),
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: false },
                has_dynamic_offset: false,
                min_binding_size: Some(Instance::NSIZE),
            },
            count: None,
        }],
    };

    pub fn new(gpu: Arc<Gpu>) -> Self {
        let instances_data = Vec::with_capacity(32);
        let instances = gpu.device().create_resizable_buffer(
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::VERTEX,
        );

        let bind_group_layout = gpu.device().create_bind_group_layout_wrap(&Self::LAYOUT);
        let bind_group = Self::create_bind_group(gpu.device(), &bind_group_layout, &instances);

        Self {
            instances_data,
            instances,
            bind_group,
            bind_group_layout,
            gpu,
        }
    }

    pub fn create_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        instances: &ResizableBuffer<Instance>,
    ) -> wgpu::BindGroup {
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Draw Instances Bind Group"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: instances.as_tight_binding(),
            }],
        });

        bind_group
    }

    pub fn add(&mut self, instances: &[Instance]) -> Vec<InstanceId> {
        let initial_len = self.instances.len();
        self.instances_data.extend_from_slice(instances);
        self.instances.push(&self.gpu, instances);
        let bind_group =
            Self::create_bind_group(self.gpu.device(), &self.bind_group_layout, &self.instances);
        self.bind_group = bind_group;

        (initial_len..)
            .take(instances.len())
            .map(|x| InstanceId(x as u32))
            .collect()
    }

    pub fn count(&self) -> u32 {
        self.instances.len() as _
    }

    pub fn clear(&mut self) {
        self.instances_data.clear();
        self.instances.clear();
    }
}
