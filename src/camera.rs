use dolly::{
    prelude::{Position, Smooth, YawPitch},
    rig::CameraRig,
};
use glam::{vec4, Mat4, Quat, Vec3, Vec4};
use wgpu::util::DeviceExt;

use crate::{
    app::bind_group_layout::{self, WrappedBindGroupLayout},
    utils::NonZeroSized,
};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    pub view_position: [f32; 4],
    pub projection: [[f32; 4]; 4],
    pub view: [[f32; 4]; 4],
    pub inv_proj: [[f32; 4]; 4],
    frustum: [f32; 4],
}

impl Default for CameraUniform {
    fn default() -> Self {
        Self {
            view_position: [0.0; 4],
            projection: Mat4::IDENTITY.to_cols_array_2d(),
            view: Mat4::IDENTITY.to_cols_array_2d(),
            inv_proj: Mat4::IDENTITY.to_cols_array_2d(),
            frustum: [0.; 4],
        }
    }
}

pub struct CameraUniformBinding {
    pub buffer: wgpu::Buffer,
    pub binding: wgpu::BindGroup,
    pub bind_group_layout: bind_group_layout::BindGroupLayout,
}

impl CameraUniformBinding {
    pub const DESC: wgpu::BindGroupLayoutDescriptor<'static> = wgpu::BindGroupLayoutDescriptor {
        label: Some("Camera Bind Group Layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT.union(wgpu::ShaderStages::COMPUTE),
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: Some(CameraUniform::NSIZE),
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
        let bind_group_layout = device.create_bind_group_layout_wrap(&Self::DESC);
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
        queue.write_buffer(&self.buffer, 0, bytemuck::bytes_of(&camera.get_uniform()));
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

    pub fn get_uniform(&self) -> CameraUniform {
        let pos = Vec4::from((self.rig.final_transform.position, 1.));
        let (projection, view) = self.build_projection_view_matrix();
        let proj_view = projection * view;

        // https://github.com/zeux/niagara/blob/3fafe000ba8fe6e309b41e915b81242b4ca3db28/src/niagara.cpp#L836-L852
        let perspective_t = projection.transpose();
        // x + w < 0
        let frustum_x = (perspective_t.col(3) + perspective_t.col(0)).normalize();
        // y + w < 0
        let frustum_y = (perspective_t.col(3) + perspective_t.col(1)).normalize();
        let frustum = vec4(frustum_x.x, frustum_x.z, frustum_y.y, frustum_y.z);

        CameraUniform {
            view_position: pos.to_array(),
            projection: projection.to_cols_array_2d(),
            view: view.to_cols_array_2d(),
            inv_proj: proj_view.inverse().to_cols_array_2d(),
            frustum: frustum.to_array(),
        }
    }
}
