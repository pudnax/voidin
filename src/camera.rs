use std::num::NonZeroU64;

use dolly::{
    prelude::{Position, Smooth, YawPitch},
    rig::CameraRig,
};
use glam::{Mat4, Quat, Vec3, Vec4};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    pub view_position: [f32; 4],
    pub projection: [[f32; 4]; 4],
    pub view: [[f32; 4]; 4],
    pub inv_proj: [[f32; 4]; 4],
}

impl Default for CameraUniform {
    fn default() -> Self {
        Self {
            view_position: [0.0; 4],
            projection: Mat4::IDENTITY.to_cols_array_2d(),
            view: Mat4::IDENTITY.to_cols_array_2d(),
            inv_proj: Mat4::IDENTITY.to_cols_array_2d(),
        }
    }
}

pub struct CameraBinding {
    pub buffer: wgpu::Buffer,
    pub binding: wgpu::BindGroup,
    pub bind_group_layout: wgpu::BindGroupLayout,
}

impl CameraBinding {
    pub const DESC: wgpu::BindGroupLayoutDescriptor<'static> = wgpu::BindGroupLayoutDescriptor {
        label: Some("Camera Bind Group Layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT.union(wgpu::ShaderStages::COMPUTE),
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: NonZeroU64::new(std::mem::size_of::<CameraUniform>() as _),
            },
            count: None,
        }],
    };

    pub fn new(device: &wgpu::Device) -> Self {
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::bytes_of(&CameraUniform::default()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bind_group_layout = device.create_bind_group_layout(&Self::DESC);
        let binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        Self {
            buffer,
            binding,
            bind_group_layout,
        }
    }

    pub fn update(&mut self, queue: &wgpu::Queue, camera: &Camera) {
        queue.write_buffer(
            &self.buffer,
            0,
            bytemuck::bytes_of(&camera.get_proj_view_matrix()),
        );
    }
}

pub struct Camera {
    pub rig: CameraRig,
    pub position: Vec3,
    pub rotation: Quat,
    pub aspect: f32,
}

impl Camera {
    const ZNEAR: f32 = 0.001;
    const FOVY: f32 = std::f32::consts::PI / 2.0;

    pub fn new(
        position: Vec3,
        yaw: f32,
        pitch: f32,
        screen_width: u32,
        screen_height: u32,
    ) -> Self {
        let rig: CameraRig = CameraRig::builder()
            .with(Position::new(position))
            .with(YawPitch::new().yaw_degrees(yaw).pitch_degrees(pitch))
            .with(Smooth::new_position_rotation(1.0, 1.5))
            .build();
        Self {
            rig,
            aspect: screen_width as f32 / screen_height as f32,
            position,
            rotation: Quat::IDENTITY,
        }
    }

    pub fn build_projection_view_matrix(&self) -> (Mat4, Mat4) {
        let tr = self.rig.final_transform;
        let view = Mat4::look_at_rh(tr.position, tr.position + tr.forward(), tr.up());
        let proj = Mat4::perspective_infinite_reverse_rh(Self::FOVY, self.aspect, Self::ZNEAR);
        (proj, view)
    }

    pub fn get_proj_view_matrix(&self) -> CameraUniform {
        let (projection, view) = self.build_projection_view_matrix();
        let proj_view = projection * view;
        let pos = Vec4::from((self.rig.final_transform.position, 1.));
        CameraUniform {
            view_position: pos.to_array(),
            projection: projection.to_cols_array_2d(),
            view: view.to_cols_array_2d(),
            inv_proj: proj_view.inverse().to_cols_array_2d(),
        }
    }
}
