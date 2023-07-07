use bytemuck::{Pod, Zeroable};
use glam::Vec3;

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, Pod, Zeroable)]
pub struct TlasNode {
    pub min: Vec3,
    pub left_right: u32,
    pub max: Vec3,
    pub blas_idx: u32,
}

impl TlasNode {
    pub fn is_leaf(&self) -> bool {
        self.left_right == 0
    }
}

pub struct Tlas {
    nodes: Vec<TlasNode>,
}

impl Tlas {
    pub fn new(instances: &[bool]) -> Self {
        Self { nodes: vec![] }
    }
}
