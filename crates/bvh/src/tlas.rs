use bytemuck::{Pod, Zeroable};
use glam::{vec3, Vec3};
use pools::{Instance, MeshInfo};

use crate::Aabb;

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
    pub fn empty() -> Self {
        Self { nodes: vec![] }
    }

    pub fn build(&mut self, instances: &[Instance], meshes: &[MeshInfo]) {
        self.nodes = vec![TlasNode::default(); 2 * instances.len()];

        // First node reserved for root
        for (i, instance) in instances.iter().enumerate().map(|(i, x)| (i + 1, x)) {
            let mesh = meshes[instance.mesh.0 as usize];
            let [min, max] = (0..8)
                .map(|i| [i & 1, i & 2, i & 4].map(|i| i == 0).map(usize::from))
                .fold(
                    [Vec3::INFINITY, Vec3::NEG_INFINITY],
                    |bound @ [min, max], [i, j, k]| {
                        let bound = instance
                            .transform
                            .transform_point3(vec3(bound[i].x, bound[j].y, bound[k].z));
                        [min.min(bound), max.max(bound)]
                    },
                );
            let node = TlasNode {
                min,
                left_right: 0,
                max,
                blas_idx: mesh.bvh_index,
            };

            self.nodes[i] = node;
        }

        let mut instance_count = instances.len();
        let mut nodes_used = 1 + instance_count;
        let mut node_indices: Vec<_> = (1..).take(instance_count).collect();
        let mut a = 0;
        let mut b = self.find_best_match(&node_indices, instance_count, a);
        while instance_count > 1 {
            let c = self.find_best_match(&node_indices, instance_count, b);
            if a == c {
                let idx_a = node_indices[a];
                let idx_b = node_indices[b];
                let node_a = &self.nodes[idx_a];
                let node_b = &self.nodes[idx_b];
                self.nodes[nodes_used] = TlasNode {
                    min: node_a.min.min(node_b.min),
                    max: node_a.max.max(node_b.max),
                    left_right: idx_a as u32 + ((idx_b as u32) << 16),
                    blas_idx: u32::MAX,
                };
                node_indices[a] = nodes_used;
                nodes_used += 1;
                node_indices[b] = node_indices[instance_count - 1];
                instance_count -= 1;
                b = self.find_best_match(&node_indices, instance_count, a);
            } else {
                a = b;
                b = c;
            }
        }
        self.nodes[0] = self.nodes[node_indices[a]];

        // TODO: untangle nodes from indices
    }

    fn find_best_match(&self, indices: &[usize], num_unused: usize, target: usize) -> usize {
        let mut smallest = 1e-30;
        let mut best_idx = target;
        for i in 0..num_unused {
            if target == i {
                continue;
            }
            let target_node = self.nodes[indices[target]];
            let best_node = self.nodes[indices[i]];
            let bmin = target_node.min.min(best_node.min);
            let bmax = target_node.max.max(best_node.max);
            let surface_area = Aabb::new(bmin, bmax).area();
            if surface_area < smallest {
                smallest = surface_area;
                best_idx = i;
            }
        }
        best_idx
    }
}
