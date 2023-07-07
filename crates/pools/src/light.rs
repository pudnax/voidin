use std::sync::Arc;

use components::{
    bind_group_layout::{self, WrappedBindGroupLayout},
    Gpu, NonZeroSized, ResizableBuffer,
};

use bytemuck::{Pod, Zeroable};
use glam::{vec3, Mat4, Vec2, Vec3, Vec3Swizzles, Vec4};

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, Pod, Zeroable)]
pub struct AreaLight {
    pub color: Vec3,
    pub intensity: f32,
    pub points: [Vec4; 4],
}

impl AreaLight {
    pub fn new(color: Vec3, intensity: f32, points: [Vec3; 4]) -> Self {
        Self {
            color,
            intensity,
            points: points.map(|v| v.extend(0.)),
        }
    }

    pub fn from_transform(color: Vec3, intensity: f32, wh: Vec2, transform: Mat4) -> Self {
        let (scale, rot, trans) = transform.to_scale_rotation_translation();
        let dir = rot.mul_vec3(vec3(0., 0., 1.)).normalize();
        let up = vec3(0., 1., 0.);
        let dirx = up.cross(dir);
        let diry = dir.cross(dirx);

        let wh = wh * scale.xy();

        let dx = dirx * wh.x / 2.;
        let dy = diry * wh.y / 2.;

        let points = [
            trans - dx - dy,
            trans + dx - dy,
            trans + dx + dy,
            trans - dx + dy,
        ];

        Self {
            color,
            intensity,
            points: points.map(|v| v.extend(0.)),
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, Pod, Zeroable)]
pub struct Light {
    pub position: glam::Vec3,
    pub radius: f32,
    pub color: glam::Vec3,
    _padding: u32,
}

impl Light {
    pub fn new(position: glam::Vec3, radius: f32, color: glam::Vec3) -> Self {
        Self {
            position,
            radius,
            color,
            _padding: 0,
        }
    }
}

pub struct LightPool {
    pub(crate) point_lights: ResizableBuffer<Light>,
    pub point_bind_group_layout: bind_group_layout::BindGroupLayout,
    pub point_bind_group: wgpu::BindGroup,

    pub(crate) area_lights: ResizableBuffer<AreaLight>,
    pub area_bind_group_layout: bind_group_layout::BindGroupLayout,
    pub area_bind_group: wgpu::BindGroup,

    gpu: Arc<Gpu>,
}

impl LightPool {
    pub fn new(gpu: Arc<Gpu>) -> Self {
        let point_lights = ResizableBuffer::new(
            gpu.device(),
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::VERTEX,
        );
        let area_lights = ResizableBuffer::new(
            gpu.device(),
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::VERTEX,
        );

        let point_bind_group_layout =
            gpu.device()
                .create_bind_group_layout_wrap(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Point Light Bind Group Layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: Some(Light::NSIZE),
                        },
                        count: None,
                    }],
                });
        let point_bind_group =
            Self::create_point_bind_group(&gpu, &point_bind_group_layout, &point_lights);

        let area_bind_group_layout =
            gpu.device()
                .create_bind_group_layout_wrap(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Area Light Bind Group Layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: Some(AreaLight::NSIZE),
                        },
                        count: None,
                    }],
                });
        let area_bind_group =
            Self::create_area_bind_group(&gpu, &area_bind_group_layout, &area_lights);

        Self {
            point_lights,
            point_bind_group_layout,
            point_bind_group,

            area_lights,
            area_bind_group_layout,
            area_bind_group,
            gpu,
        }
    }

    // FIXME: sets `arrayLength` to 32 if the buffer is empty
    fn create_point_bind_group(
        gpu: &Gpu,
        bind_group_layout: &wgpu::BindGroupLayout,
        lights: &ResizableBuffer<Light>,
    ) -> wgpu::BindGroup {
        gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Point Light Pool Bind Group"),
            layout: bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: lights.as_tight_binding(),
            }],
        })
    }

    fn create_area_bind_group(
        gpu: &Gpu,
        bind_group_layout: &wgpu::BindGroupLayout,
        lights: &ResizableBuffer<AreaLight>,
    ) -> wgpu::BindGroup {
        gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Area Light Pool Bind Group"),
            layout: bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: lights.as_tight_binding(),
            }],
        })
    }

    pub fn add_point_light(&mut self, lights: &[Light]) {
        self.point_lights.push(&self.gpu, lights);
        self.point_bind_group = Self::create_point_bind_group(
            &self.gpu,
            &self.point_bind_group_layout,
            &self.point_lights,
        );
    }

    pub fn add_area_light(&mut self, lights: &[AreaLight]) {
        self.area_lights.push(&self.gpu, lights);
        self.area_bind_group = Self::create_area_bind_group(
            &self.gpu,
            &self.area_bind_group_layout,
            &self.area_lights,
        );
    }
}
