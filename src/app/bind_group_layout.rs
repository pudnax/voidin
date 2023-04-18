use std::{
    hash::Hash,
    marker::PhantomData,
    num::NonZeroU32,
    ops::Deref,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
};

use crate::{utils::NonZeroSized, Gpu};
use pretty_type_name::pretty_type_name;

#[repr(transparent)]
#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
pub struct BindGroupLayoutId(NonZeroU32);

impl BindGroupLayoutId {
    fn new() -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(1);

        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        Self(NonZeroU32::new(counter).unwrap_or_else(|| {
            panic!(
                "The system ran out of unique `{}`s.",
                pretty_type_name::<Self>(),
            );
        }))
    }
}

#[derive(Clone, Debug)]
pub struct StorageReadBindGroupLayout<T> {
    layout: BindGroupLayout,
    _marker: PhantomData<T>,
}

impl<T> Deref for StorageReadBindGroupLayout<T> {
    type Target = BindGroupLayout;
    fn deref(&self) -> &Self::Target {
        &self.layout
    }
}

impl<T: NonZeroSized> StorageReadBindGroupLayout<T> {
    pub fn new(gpu: &Gpu) -> Self {
        let layout = gpu
            .device()
            .create_bind_group_layout_wrap(&wgpu::BindGroupLayoutDescriptor {
                label: Some(&format!(
                    "Read Buffer<{}> Bind Group Layout",
                    pretty_type_name::<T>()
                )),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE
                        .union(wgpu::ShaderStages::VERTEX_FRAGMENT),
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: Some(T::NSIZE),
                    },
                    count: None,
                }],
            });
        Self {
            layout,
            _marker: PhantomData,
        }
    }
}

#[derive(Clone, Debug)]
pub struct StorageWriteBindGroupLayout<T> {
    pub layout: BindGroupLayout,
    _marker: PhantomData<T>,
}

impl<T> Deref for StorageWriteBindGroupLayout<T> {
    type Target = BindGroupLayout;
    fn deref(&self) -> &Self::Target {
        &self.layout
    }
}

impl<T: NonZeroSized> StorageWriteBindGroupLayout<T> {
    pub fn new(gpu: &Gpu) -> Self {
        let layout = gpu
            .device()
            .create_bind_group_layout_wrap(&wgpu::BindGroupLayoutDescriptor {
                label: Some(&format!(
                    "Write Buffer<{}> Bind Group Layout",
                    pretty_type_name::<T>()
                )),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE
                        .union(wgpu::ShaderStages::VERTEX_FRAGMENT),
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: Some(T::NSIZE),
                    },
                    count: None,
                }],
            });
        Self {
            layout,
            _marker: PhantomData,
        }
    }
}

#[derive(Clone, Debug)]
pub struct StorageWriteBindGroupLayoutDyn(pub BindGroupLayout);

impl Deref for StorageWriteBindGroupLayoutDyn {
    type Target = BindGroupLayout;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl StorageWriteBindGroupLayoutDyn {
    pub fn new(gpu: &Gpu) -> Self {
        let layout = gpu
            .device()
            .create_bind_group_layout_wrap(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Write Buffer Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE
                        .union(wgpu::ShaderStages::VERTEX_FRAGMENT),
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        Self(layout)
    }
}

#[derive(Clone, Debug)]
pub struct StorageReadBindGroupLayoutDyn(pub BindGroupLayout);

impl Deref for StorageReadBindGroupLayoutDyn {
    type Target = BindGroupLayout;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl StorageReadBindGroupLayoutDyn {
    pub fn new(gpu: &Gpu) -> Self {
        let layout = gpu
            .device()
            .create_bind_group_layout_wrap(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Read Buffer Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE
                        .union(wgpu::ShaderStages::VERTEX_FRAGMENT),
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        Self(layout)
    }
}

#[derive(Clone, Debug)]
pub struct BindGroupLayout {
    id: BindGroupLayoutId,
    value: Arc<wgpu::BindGroupLayout>,
}

impl BindGroupLayout {
    pub fn new(layout: wgpu::BindGroupLayout) -> Self {
        Self {
            id: BindGroupLayoutId::new(),
            value: Arc::new(layout),
        }
    }
}

impl PartialEq for BindGroupLayout {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for BindGroupLayout {}

impl Hash for BindGroupLayout {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl BindGroupLayout {
    pub fn id(&self) -> BindGroupLayoutId {
        self.id
    }

    pub fn value(&self) -> &wgpu::BindGroupLayout {
        &self.value
    }
}

impl From<wgpu::BindGroupLayout> for BindGroupLayout {
    fn from(value: wgpu::BindGroupLayout) -> Self {
        BindGroupLayout {
            id: BindGroupLayoutId::new(),
            value: Arc::new(value),
        }
    }
}

impl Deref for BindGroupLayout {
    type Target = wgpu::BindGroupLayout;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

pub trait WrappedBindGroupLayout {
    fn create_bind_group_layout_wrap(
        &self,
        desc: &wgpu::BindGroupLayoutDescriptor,
    ) -> BindGroupLayout;
}

impl WrappedBindGroupLayout for wgpu::Device {
    fn create_bind_group_layout_wrap(
        &self,
        desc: &wgpu::BindGroupLayoutDescriptor,
    ) -> BindGroupLayout {
        let layout = self.create_bind_group_layout(desc);
        BindGroupLayout::new(layout)
    }
}
