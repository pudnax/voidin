use std::path::Path;

use color_eyre::Result;
use wgpu::util::align_to;

use crate::{
    app::{
        bind_group_layout::StorageReadBindGroupLayout,
        global_ubo::GlobalUniformBinding,
        instance::InstancePool,
        pipeline::{ComputeHandle, ComputePipelineDescriptor, PipelineArena},
        ViewTarget,
    },
    utils::world::World,
};

use super::Pass;

pub struct ComputeUpdate {
    pipeline: ComputeHandle,
}

impl ComputeUpdate {
    pub fn new(world: &World, path: impl AsRef<Path>) -> Result<Self> {
        let global_ubo = world.get::<GlobalUniformBinding>()?;
        let read_idx_layout = world.get::<StorageReadBindGroupLayout<u32>>()?;
        let instances = world.get::<InstancePool>()?;
        let desc = ComputePipelineDescriptor {
            label: Some("Compute Geometry Update Pass".into()),
            layout: vec![
                global_ubo.layout.clone(),
                read_idx_layout.layout.clone(),
                instances.bind_group_layout.clone(),
            ],
            push_constant_ranges: vec![],
            entry_point: "update".into(),
        };
        let pipeline = world
            .get_mut::<PipelineArena>()?
            .process_compute_pipeline_from_path(path, desc)?;
        Ok(Self { pipeline })
    }
}

pub struct ComputeUpdateResourse<'a> {
    pub idx_bind_group: &'a wgpu::BindGroup,
    pub dispatch_size: u32,
}

impl Pass for ComputeUpdate {
    type Resoutces<'a> = ComputeUpdateResourse<'a>;

    fn record(
        &self,
        world: &World,
        encoder: &mut wgpu::CommandEncoder,
        _view_target: &ViewTarget,
        resources: Self::Resoutces<'_>,
    ) {
        let arena = world.unwrap::<PipelineArena>();
        let instances = world.unwrap::<InstancePool>();
        let global_ubo = world.unwrap::<GlobalUniformBinding>();
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Compute Update Pass"),
        });

        cpass.set_pipeline(arena.get_pipeline(self.pipeline));
        cpass.set_bind_group(0, &global_ubo.binding, &[]);
        cpass.set_bind_group(1, resources.idx_bind_group, &[]);
        cpass.set_bind_group(2, &instances.bind_group, &[]);
        let num_dispatches = align_to(resources.dispatch_size, 1) / 1;
        cpass.dispatch_workgroups(num_dispatches, 1, 1);
    }
}
