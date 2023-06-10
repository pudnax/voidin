use app::{
    pipeline::{self, PipelineArena, RenderHandle, VertexState},
    run, App, AppState, Example, Gpu, RenderContext,
};
use color_eyre::Result;

struct Triangle {
    pipeline: RenderHandle,
}

impl Example for Triangle {
    fn name() -> &'static str {
        "Triangle"
    }

    fn init(app: &mut App) -> Result<Self> {
        let pipeline = app
            .get_pipeline_arena_mut()
            .process_render_pipeline_from_path(
                "shaders/trig.wgsl",
                pipeline::RenderPipelineDescriptor {
                    vertex: VertexState {
                        entry_point: "vs_main_trig".into(),
                        ..Default::default()
                    },
                    depth_stencil: None,
                    ..Default::default()
                },
            )?;
        Ok(Self { pipeline })
    }

    fn update(&mut self, _app: &App, _app_state: &AppState) {}

    fn resize(&mut self, _gpu: &Gpu, _width: u32, _height: u32) {}

    fn render(&self, mut ctx: RenderContext) {
        let arena = ctx.world.unwrap::<PipelineArena>();
        let mut pass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: ctx.view_target.main_view(),
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.13,
                        g: 0.13,
                        b: 0.13,
                        a: 1.0,
                    }),
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
        });

        pass.set_pipeline(arena.get_pipeline(self.pipeline));
        pass.draw(0..3, 0..1);
    }
}

fn main() -> Result<()> {
    run::<Triangle>()
}
