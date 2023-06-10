use std::path::Path;

use app::{
    models, run, AppState, CameraUniform, Example, GltfDocument, Gpu, Instance, MaterialId,
    ResizableBuffer, ResizableBufferExt, {self, InstanceId, InstancePool}, {App, RenderContext},
    {Light, LightPool},
};
use color_eyre::Result;
use glam::{vec3, Mat4, Vec3};
use pass::Pass;
use rand::Rng;

struct Model {
    visibility_pass: pass::visibility::Visibility,
    emit_draws_pass: pass::visibility::EmitDraws,

    shading_pass: pass::shading::ShadingPass,

    postprocess_pass: pass::postprocess::PostProcess,

    update_pass: pass::compute_update::ComputeUpdate,

    taa_pass: pass::taa::Taa,

    moving_instances: ResizableBuffer<InstanceId>,
    moving_instances_bind_group: wgpu::BindGroup,
}

impl Example for Model {
    fn name() -> &'static str {
        "Model"
    }

    fn init(app: &mut App) -> Result<Self> {
        let visibility_pass = pass::visibility::Visibility::new(&app.world)?;
        let emit_draws_pass = pass::visibility::EmitDraws::new(&app.world)?;

        let shading_pass = pass::shading::ShadingPass::new(&app.world, &app.gbuffer)?;

        let path = Path::new("shaders").join("postprocess.wgsl");
        let postprocess_pass = pass::postprocess::PostProcess::new(&app.world, path)?;

        let path = Path::new("shaders").join("compute_update.wgsl");
        let update_pass = pass::compute_update::ComputeUpdate::new(&app.world, path)?;

        let taa_pass = pass::taa::Taa::new(
            &app.world,
            &app.gbuffer,
            app.surface_config.width,
            app.surface_config.height,
        )?;
        let moving_instances = app
            .device()
            .create_resizable_buffer(wgpu::BufferUsages::STORAGE);
        let moving_instances_bind_group =
            moving_instances.create_storage_read_bind_group(&mut app.world);

        Ok(Self {
            visibility_pass,
            emit_draws_pass,
            shading_pass,
            postprocess_pass,
            update_pass,
            taa_pass,

            moving_instances,
            moving_instances_bind_group,
        })
    }

    fn setup_scene(&mut self, app: &mut App) -> Result<()> {
        use std::f32::consts::PI;
        let mut instances = vec![];

        app.world
            .get_mut::<LightPool>()?
            .add_point_light(&[Light::new(vec3(0., 0.5, 0.), 10., vec3(1., 1., 1.))]);

        app.add_area_light(
            vec3(1., 1., 1.),
            7.,
            (5., 8.).into(),
            Mat4::from_translation(vec3(0., 10., 15.)) * Mat4::from_rotation_x(-PI / 4.),
        )?;
        app.add_area_light(
            vec3(1., 1., 1.),
            7.,
            (5., 8.).into(),
            Mat4::from_translation(vec3(0., 10., -25.)) * Mat4::from_rotation_x(-3. * PI / 4.),
        )?;

        let gltf_scene = GltfDocument::import(
            app,
            "assets/glTF-Sample-Models/2.0/Sponza/glTF/Sponza.gltf",
            // "assets/glTF-Sample-Models/2.0/AntiqueCamera/glTF/AntiqueCamera.gltf",
            // "assets/glTF-Sample-Models/2.0/Buggy/glTF-Binary/Buggy.glb",
            // "assets/glTF-Sample-Models/2.0/FlightHelmet/glTF/FlightHelmet.gltf",
            // "assets/glTF-Sample-Models/2.0/DamagedHelmet/glTF-Binary/DamagedHelmet.glb",
        )?;

        instances.extend(gltf_scene.get_scene_instances(
            Mat4::from_rotation_y(PI / 2.)
                * Mat4::from_translation(vec3(7., -5., 1.))
                * Mat4::from_scale(Vec3::splat(3.)),
        ));

        let helmet = GltfDocument::import(
            app,
            "assets/glTF-Sample-Models/2.0/DamagedHelmet/glTF-Binary/DamagedHelmet.glb",
        )?;
        instances.extend(helmet.get_scene_instances(
            Mat4::from_translation(vec3(0., 0., 9.)) * Mat4::from_scale(Vec3::splat(3.)),
        ));

        let gltf_ferris = GltfDocument::import(app, "assets/ferris3d_v1.0.glb")?;
        instances.extend(gltf_ferris.get_scene_instances(
            Mat4::from_translation(vec3(-3., -5.0, -4.)) * Mat4::from_scale(Vec3::splat(3.)),
        ));
        instances.extend(gltf_ferris.get_scene_instances(
            Mat4::from_translation(vec3(2., -5.0, -2.)) * Mat4::from_scale(Vec3::splat(3.)),
        ));
        gltf_ferris.get_scene_instances(
            Mat4::from_translation(vec3(2., -5.0, -2.)) * Mat4::from_scale(Vec3::splat(3.)),
        );
        app.world.get_mut::<InstancePool>()?.add(&instances);

        let sphere_mesh = models::make_uv_sphere(1.0, 10);
        let sphere_mesh_id = app.get_mesh_pool_mut().add(sphere_mesh.as_ref());

        let mut moving_instances = vec![];
        let mut rng = rand::thread_rng();
        let num = 10;
        for i in 0..num {
            let r = 3.5;
            let angle = 2. * PI * (i as f32) / num as f32;
            let x = r * angle.cos();
            let y = r * angle.sin();

            moving_instances.push(Instance::new(
                Mat4::from_translation(vec3(x, y, -17.)),
                sphere_mesh_id,
                MaterialId::new(rng.gen_range(0..app.get_material_pool().num_materials() as u32)),
            ));

            moving_instances.extend(gltf_ferris.get_scene_instances(
                Mat4::from_translation(vec3(x, y + 0., -9.))
                    * Mat4::from_rotation_z(angle)
                    * Mat4::from_scale(Vec3::splat(2.5)),
            ));
        }

        let moving_instances_id = app.world.get_mut::<InstancePool>()?.add(&moving_instances);
        self.moving_instances.push(&app.gpu, &moving_instances_id);
        self.moving_instances_bind_group = self
            .moving_instances
            .create_storage_read_bind_group(&mut app.world);

        Ok(())
    }

    fn update(&mut self, app: &App, app_state: &AppState) {
        let jitter = self.taa_pass.get_jitter(
            app_state.frame_count as u32,
            app.surface_config.width,
            app.surface_config.height,
        );
        let mut camera_uniform = app.world.get_mut::<CameraUniform>().unwrap();
        *camera_uniform = app_state
            .camera
            .get_uniform(Some(jitter.to_array()), Some(&camera_uniform));

        let mut encoder = app
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Compute Update"),
            });
        let resources = pass::compute_update::ComputeUpdateResourse {
            idx_bind_group: &self.moving_instances_bind_group,
            dispatch_size: self.moving_instances.len() as u32,
        };
        self.update_pass.record(&app.world, &mut encoder, resources);
        app.queue().submit(Some(encoder.finish()));
    }

    fn resize(&mut self, gpu: &Gpu, width: u32, height: u32) {
        self.taa_pass.resize(gpu.device(), width, height);
    }

    fn render(
        &self,
        RenderContext {
            mut encoder,
            world,
            gbuffer,
            view_target,
            draw_cmd_bind_group,
            draw_cmd_buffer,
            width,
            height,
            ..
        }: RenderContext,
    ) {
        encoder.profile_start("Visibility");
        self.emit_draws_pass.record(
            &world,
            &mut encoder,
            pass::visibility::EmitDrawsResource {
                draw_cmd_bind_group: &draw_cmd_bind_group,
                draw_cmd_buffer: &draw_cmd_buffer,
            },
        );

        self.visibility_pass.record(
            &world,
            &mut encoder,
            pass::visibility::VisibilityResource {
                gbuffer: &gbuffer,
                draw_cmd_buffer: &draw_cmd_buffer,
            },
        );
        encoder.profile_end();

        self.shading_pass.record(
            &world,
            &mut encoder,
            pass::shading::ShadingResource {
                gbuffer: &gbuffer,
                view_target: &view_target,
            },
        );

        self.taa_pass.record(
            &world,
            &mut encoder,
            pass::taa::TaaResource {
                gbuffer: &gbuffer,
                view_target: &view_target,
                width_height: (width, height),
            },
        );

        self.postprocess_pass.record(
            &world,
            &mut encoder,
            pass::postprocess::PostProcessResource {
                view_target: &view_target,
            },
        );
    }
}

fn main() -> Result<()> {
    run::<Model>()
}
