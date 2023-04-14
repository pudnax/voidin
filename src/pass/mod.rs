pub mod geometry;
pub mod postprocess;

pub trait Pass {
    type Resoutces<'a>;

    fn record(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view_target: &crate::app::ViewTarget,
        resources: Self::Resoutces<'_>,
    );
}
