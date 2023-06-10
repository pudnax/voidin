use components::world::World;

pub mod compute_update;
pub mod postprocess;
pub mod shading;
pub mod taa;
pub mod visibility;

pub trait Pass {
    type Resources<'a>;

    fn record(
        &self,
        world: &World,
        encoder: &mut wgpu::CommandEncoder,
        resources: Self::Resources<'_>,
    );
}
