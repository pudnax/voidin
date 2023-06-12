use crate::{
    bind_group_layout::{StorageReadBindGroupLayout, StorageWriteBindGroupLayout},
    Gpu,
};

use std::{marker::PhantomData, ops::RangeBounds};

use bytemuck::Pod;
use pretty_type_name::pretty_type_name;
use wgpu::{
    util::DeviceExt, Buffer, BufferAddress, BufferDescriptor, BufferSlice, BufferUsages,
    CommandEncoder, CommandEncoderDescriptor, Device,
};

use super::{world::World, NonZeroSized};

pub trait ResizableBufferExt {
    fn create_resizable_buffer<T: Pod>(&self, usages: BufferUsages) -> ResizableBuffer<T>;

    fn create_resizable_buffer_init<T: Pod>(
        &self,
        usages: BufferUsages,
        data: &[T],
    ) -> ResizableBuffer<T>;
}

impl ResizableBufferExt for wgpu::Device {
    fn create_resizable_buffer<T: Pod>(&self, usages: BufferUsages) -> ResizableBuffer<T> {
        ResizableBuffer::new(self, usages)
    }

    fn create_resizable_buffer_init<T: Pod>(
        &self,
        usages: BufferUsages,
        data: &[T],
    ) -> ResizableBuffer<T> {
        ResizableBuffer::new_with_data(self, usages, data)
    }
}

#[derive(Debug)]
pub struct ResizableBuffer<T> {
    buffer: Buffer,
    len: usize,
    cap: usize,
    _phantom: PhantomData<T>,
}

impl<T> std::ops::Deref for ResizableBuffer<T> {
    type Target = Buffer;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

impl<T: bytemuck::Pod + NonZeroSized> ResizableBuffer<T> {
    pub fn new(device: &Device, usages: BufferUsages) -> Self {
        let default_cap = 32;
        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some(&format!("Buffer<{}>", pretty_type_name::<T>())),
            size: (T::SIZE * default_cap) as u64,
            usage: usages | BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            buffer,

            len: 0,
            cap: default_cap,
            _phantom: PhantomData,
        }
    }

    pub fn new_with_data(device: &Device, usages: BufferUsages, data: &[T]) -> Self {
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("Buffer<{}>", pretty_type_name::<T>())),
            contents: bytemuck::cast_slice(data),
            usage: usages | BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
        });

        Self {
            buffer,

            len: data.len(),
            cap: data.len(),
            _phantom: PhantomData,
        }
    }

    pub fn reserve(
        &mut self,
        device: &Device,
        encoder: &mut CommandEncoder,
        new_len: usize,
    ) -> bool {
        if new_len < self.cap {
            return false;
        }

        let max_buffer_size = device.limits().max_buffer_size;
        let new_cap = (new_len + 1)
            .checked_next_power_of_two()
            .unwrap_or(new_len)
            .min(max_buffer_size as usize / T::SIZE);
        let new_buf = device.create_buffer(&BufferDescriptor {
            label: Some(&format!("Buffer<{}>", pretty_type_name::<T>())),
            size: (T::SIZE * new_cap) as u64,
            usage: self.usages(),
            mapped_at_creation: false,
        });

        let old = std::mem::replace(&mut self.buffer, new_buf);

        encoder.copy_buffer_to_buffer(&old, 0, &self.buffer, 0, self.size_bytes());
        self.cap = new_cap;

        true
    }

    pub fn set_len(
        &mut self,
        device: &Device,
        encoder: &mut CommandEncoder,
        new_len: usize,
    ) -> bool {
        let was_reallocated = self.reserve(device, encoder, new_len);
        self.len = new_len;
        was_reallocated
    }

    /// Returns `true` if internal buffer was resized
    pub fn push(&mut self, gpu: &Gpu, values: &[T]) -> bool {
        assert!(!values.is_empty(), "Don't push empty values");
        let new_len = self.len() + values.len();
        let mut encoder = gpu
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Copy Buffer Encoder"),
            });

        let was_reallocated = self.reserve(&gpu.device, &mut encoder, new_len);
        gpu.queue.submit(Some(encoder.finish()));

        gpu.queue.write_buffer(
            &self.buffer,
            self.size_bytes(),
            bytemuck::cast_slice(values),
        );
        self.len = new_len;
        was_reallocated
    }

    pub fn pop(&mut self) {
        assert!(!self.is_empty(), "Attempted to pop empty buffer");
        self.len -= 1;
    }

    pub fn write(&mut self, gpu: &Gpu, index: usize, value: T) {
        assert!(index < self.len());
        gpu.queue.write_buffer(
            &self.buffer,
            (index * T::SIZE) as BufferAddress,
            bytemuck::bytes_of(&value),
        );
    }

    pub fn write_slice(&mut self, gpu: &Gpu, index: usize, values: &[T]) {
        assert!(index + values.len() <= self.len());
        gpu.queue.write_buffer(
            &self.buffer,
            (index * T::SIZE) as BufferAddress,
            bytemuck::cast_slice(values),
        );
    }

    pub fn write_bytes(&mut self, gpu: &Gpu, offset: BufferAddress, bytes: &[u8]) {
        gpu.queue.write_buffer(&self.buffer, offset, bytes);
    }

    pub fn read(&self, gpu: &Gpu) -> Vec<T> {
        let staging = gpu.device().create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: self.size_bytes(),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = gpu.device().create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(&self.buffer, 0, &staging, 0, self.size_bytes());
        let submit = gpu.queue().submit(Some(encoder.finish()));
        let slice = staging.slice(..);
        slice.map_async(wgpu::MapMode::Read, |err| {
            if let Err(err) = err {
                log::error!("Failed to map buffer: {err}");
            }
        });
        gpu.device()
            .poll(wgpu::Maintain::WaitForSubmissionIndex(submit));
        let mapped = slice.get_mapped_range();
        bytemuck::cast_slice(&mapped).to_vec()
    }

    pub fn create_storage_read_bind_group(&self, world: &mut World) -> wgpu::BindGroup {
        let gpu = world.gpu.clone();
        let layout = world
            .entry::<StorageReadBindGroupLayout<T>>()
            .or_insert_with(|world| StorageReadBindGroupLayout::new(&world.gpu));

        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("Buffer<{}> Bind Group", pretty_type_name::<T>())),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.as_tight_binding(),
            }],
        })
    }

    pub fn create_storage_write_bind_group(&self, world: &mut World) -> wgpu::BindGroup {
        let gpu = world.gpu.clone();
        let layout = world
            .entry::<StorageWriteBindGroupLayout<T>>()
            .or_insert_with(|world| StorageWriteBindGroupLayout::new(&world.gpu));

        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("Buffer<{}> Bind Group", pretty_type_name::<T>())),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.as_tight_binding(),
            }],
        })
    }

    pub fn as_tight_binding(&self) -> wgpu::BindingResource {
        wgpu::BindingResource::Buffer(self.as_tight_buffer_binding())
    }

    pub fn as_entire_binding(&self) -> wgpu::BindingResource {
        wgpu::BindingResource::Buffer(self.as_entire_buffer_binding())
    }

    pub fn as_tight_buffer_binding(&self) -> wgpu::BufferBinding {
        wgpu::BufferBinding {
            buffer: &self.buffer,
            offset: 0,
            size: std::num::NonZeroU64::new(self.size_bytes()),
        }
    }

    pub fn as_entire_buffer_binding(&self) -> wgpu::BufferBinding {
        wgpu::BufferBinding {
            buffer: &self.buffer,
            offset: 0,
            size: None,
        }
    }

    pub fn usages(&self) -> BufferUsages {
        self.buffer.usage()
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }

    pub fn size_bytes(&self) -> BufferAddress {
        (T::SIZE * self.len) as BufferAddress
    }

    pub fn slice<S: RangeBounds<BufferAddress>>(&self, bounds: S) -> BufferSlice {
        self.buffer.slice(bounds)
    }

    pub fn full_slice(&self) -> BufferSlice {
        self.slice(0..self.size_bytes())
    }
}
