use crate::utils::world::World;

pub mod compute_update;
pub mod postprocess;
pub mod shading;
pub mod visibility;

pub trait Pass {
    type Resoutces<'a>;

    fn record(
        &self,
        world: &World,
        encoder: &mut wgpu::CommandEncoder,
        resources: Self::Resoutces<'_>,
    );
}
