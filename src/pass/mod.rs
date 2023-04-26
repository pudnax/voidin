use crate::utils::world::World;

pub mod ambient;
pub mod compute_update;
pub mod geometry;
pub mod light;
pub mod postprocess;

pub trait Pass {
    type Resoutces<'a>;

    fn record(
        &self,
        world: &World,
        encoder: &mut wgpu::CommandEncoder,
        resources: Self::Resoutces<'_>,
    );
}
