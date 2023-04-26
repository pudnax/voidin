use std::path::Path;

use color_eyre::Result;

use crate::{
    app::{
        gbuffer::GBuffer,
        global_ubo::GlobalUniformBinding,
        light::LightPool,
        material::MaterialPool,
        pipeline::{PipelineArena, RenderHandle, RenderPipelineDescriptor},
        texture::TexturePool,
    },
    camera::CameraUniformBinding,
    utils::world::World,
};

use super::Pass;

pub struct ShadingPass {
    pipeline: RenderHandle,
}

impl ShadingPass {
    pub fn new(world: &World, gbuffer: &GBuffer) -> Result<Self> {
        let global = world.get::<GlobalUniformBinding>()?;
        let camera = world.get::<CameraUniformBinding>()?;
        let materials = world.get::<MaterialPool>()?;
        let textures = world.get::<TexturePool>()?;
        let lights = world.get::<LightPool>()?;
        let shader_path = Path::new("shaders").join("shading.wgsl");
        let desc = RenderPipelineDescriptor {
            label: Some("Shading Pipeline".into()),
            layout: vec![
                global.layout.clone(),
                camera.bind_group_layout.clone(),
                gbuffer.bind_group_layout.clone(),
                textures.bind_group_layout.clone(),
                materials.bind_group_layout.clone(),
                lights.bind_group_layout.clone(),
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

pub struct ShadingResource<'a> {
    pub gbuffer: &'a GBuffer,
    pub view_target: &'a crate::app::ViewTarget,
}

impl Pass for ShadingPass {
    type Resoutces<'a> = ShadingResource<'a>;

    fn record(
        &self,
        world: &World,
        encoder: &mut wgpu::CommandEncoder,
        resources: Self::Resoutces<'_>,
    ) {
        let global = world.unwrap::<GlobalUniformBinding>();
        let camera = world.unwrap::<CameraUniformBinding>();
        let arena = world.unwrap::<PipelineArena>();
        let textures = world.unwrap::<TexturePool>();
        let materials = world.unwrap::<MaterialPool>();
        let lights = world.unwrap::<LightPool>();

        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Shading Pass"),
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
        rpass.set_bind_group(0, &global.binding, &[]);
        rpass.set_bind_group(1, &camera.binding, &[]);
        rpass.set_bind_group(2, &resources.gbuffer.bind_group, &[]);
        rpass.set_bind_group(3, &textures.bind_group, &[]);
        rpass.set_bind_group(4, &materials.bind_group, &[]);
        rpass.set_bind_group(5, &lights.bind_group, &[]);

        rpass.draw(0..3, 0..1);
    }
}
