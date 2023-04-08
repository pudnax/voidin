use std::{
    hash::Hash,
    num::NonZeroU32,
    ops::Deref,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
};

#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
pub struct BindGroupLayoutId(NonZeroU32);

impl BindGroupLayoutId {
    fn new() -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(1);

        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        Self(NonZeroU32::new(counter).unwrap_or_else(|| {
            panic!(
                "The system ran out of unique `{}`s.",
                std::any::type_name::<Self>(),
            );
        }))
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
