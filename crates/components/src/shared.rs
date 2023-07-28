use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct MeshId(pub u32);

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
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    pub fn id(&self) -> u32 {
        self.0
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Pod, Zeroable)]
pub struct MeshInfo {
    pub min: Vec3,
    pub index_count: u32,
    pub max: Vec3,
    pub base_index: u32,
    pub vertex_offset: i32,
    pub bvh_index: u32,
    pub junk: [u32; 2],
}

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Pod, Zeroable)]
pub struct MaterialId(pub u32);

impl MaterialId {
    pub const fn new(id: u32) -> Self {
        Self(id)
    }
}

impl Default for MaterialId {
    fn default() -> Self {
        Self(1)
    }
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Zeroable, Pod)]
pub struct InstanceId(pub u32);

impl InstanceId {
    pub fn id(&self) -> u32 {
        self.0
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct Instance {
    pub transform: glam::Mat4,
    inv_transform: glam::Mat4,
    pub mesh: MeshId,
    pub material: MaterialId,
    junk: [u32; 2],
}

impl Default for Instance {
    fn default() -> Self {
        Self {
            transform: Mat4::IDENTITY,
            inv_transform: Mat4::IDENTITY,
            mesh: MeshId::default(),
            material: MaterialId::default(),
            junk: [0; 2],
        }
    }
}

impl Instance {
    pub fn new(transform: glam::Mat4, mesh: MeshId, material: MaterialId) -> Self {
        Self {
            transform,
            inv_transform: transform.inverse(),
            mesh,
            material,
            junk: [0; 2],
        }
    }

    pub fn transform(&mut self, transform: glam::Mat4) {
        self.transform = transform * self.transform;
    }
}
