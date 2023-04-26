use std::path::Path;

use color_eyre::Result;

use crate::{
    app::{
        gbuffer::GBuffer,
        material::MaterialPool,
        pipeline::{PipelineArena, RenderHandle, RenderPipelineDescriptor},
        texture::TexturePool,
    },
    utils::world::World,
};

use super::Pass;

pub struct AmbientPass {
    pipeline: RenderHandle,
}

impl AmbientPass {
    pub fn new(world: &World, gbuffer: &GBuffer) -> Result<Self> {
        let materials = world.get::<MaterialPool>()?;
        let textures = world.get::<TexturePool>()?;
        let shader_path = Path::new("shaders").join("ambient.wgsl");
        let desc = RenderPipelineDescriptor {
            label: Some("Ambient Light Pipeline".into()),
            layout: vec![
                gbuffer.bind_group_layout.clone(),
                textures.bind_group_layout.clone(),
                materials.bind_group_layout.clone(),
            ],
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
    pub view_target: &'a crate::app::ViewTarget,
}

impl Pass for AmbientPass {
    type Resoutces<'a> = AmbientResource<'a>;

    fn record(
        &self,
        world: &World,
        encoder: &mut wgpu::CommandEncoder,
        resources: Self::Resoutces<'_>,
    ) {
        let arena = world.unwrap::<PipelineArena>();
        let textures = world.unwrap::<TexturePool>();
        let materials = world.unwrap::<MaterialPool>();

        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Ambient Light"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: resources.view_target.main_view(),
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
        rpass.set_bind_group(1, &textures.bind_group, &[]);
        rpass.set_bind_group(2, &materials.bind_group, &[]);

        rpass.draw(0..3, 0..1);
    }
}
