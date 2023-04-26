use std::path::Path;

use color_eyre::Result;

use crate::{
    app::{
        gbuffer::GBuffer,
        pipeline::{PipelineArena, RenderHandle, RenderPipelineDescriptor},
    },
    utils::world::World,
};

use super::Pass;

pub struct AmbientPass {
    pipeline: RenderHandle,
}

impl AmbientPass {
    pub fn new(world: &World, gbuffer: &GBuffer) -> Result<Self> {
        let shader_path = Path::new("shaders").join("ambient.wgsl");
        let desc = RenderPipelineDescriptor {
            label: Some("Ambient Light Pipeline".into()),
            layout: vec![gbuffer.bind_group_layout.clone()],
            depth_stencil: None,
            ..Default::default()
        };
        let pipeline = world
            .get_mut::<PipelineArena>()?
            .process_render_pipeline_from_path(shader_path, desc)?;
        Ok(Self { pipeline })
    }
}

pub struct AmbientResource<'a> {
    pub gbuffer: &'a GBuffer,
}

impl Pass for AmbientPass {
    type Resoutces<'a> = AmbientResource<'a>;

    fn record(
        &self,
        world: &World,
        encoder: &mut wgpu::CommandEncoder,
        view_target: &crate::app::ViewTarget,
        resources: Self::Resoutces<'_>,
    ) {
        let arena = world.unwrap::<PipelineArena>();

        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Ambient Light"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: view_target.main_view(),
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
        });

        rpass.set_pipeline(arena.get_pipeline(self.pipeline));
        rpass.set_bind_group(0, &resources.gbuffer.bind_group, &[]);

        rpass.draw(0..3, 0..1);
    }
}
