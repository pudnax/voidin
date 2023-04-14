use std::{marker::PhantomData, mem::size_of, num::NonZeroU64};

use bytemuck::Pod;
use wgpu::{
    util::DeviceExt, Buffer, BufferAddress, BufferDescriptor, BufferSlice, BufferUsages,
    CommandEncoder, CommandEncoderDescriptor, Device, Queue,
};

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

impl<T: bytemuck::Pod> ResizableBuffer<T> {
    pub fn new(device: &Device, usages: BufferUsages) -> Self {
        let default_cap = 32;
        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some(std::any::type_name::<Self>()),
            size: (size_of::<T>() * default_cap) as u64,
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
            label: Some(std::any::type_name::<Self>()),
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
        let new_cap = new_len
            .checked_next_power_of_two()
            .unwrap_or(new_len)
            .min(max_buffer_size as usize / size_of::<T>());
        let new_buf = device.create_buffer(&BufferDescriptor {
            label: Some(std::any::type_name::<Self>()),
            size: (size_of::<T>() * new_cap) as u64,
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
    // TODO: Consider adding on_resize callback
    pub fn push(&mut self, device: &Device, queue: &Queue, values: &[T]) -> bool {
        let new_len = self.len() + values.len();
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Copy Buffer Encoder"),
        });

        let was_reallocated = self.reserve(device, &mut encoder, new_len);

        queue.write_buffer(
            &self.buffer,
            dbg!(self.size_bytes()),
            bytemuck::cast_slice(values),
        );
        self.len = new_len;
        was_reallocated
    }

    pub fn write(&mut self, queue: &Queue, index: usize, value: T) {
        assert!(index < self.len());
        queue.write_buffer(
            &self.buffer,
            (index * size_of::<T>()) as BufferAddress,
            bytemuck::bytes_of(&value),
        );
    }

    pub fn write_slice(&mut self, queue: &Queue, index: usize, values: &[T]) {
        assert!(index + values.len() < self.len());
        queue.write_buffer(
            &self.buffer,
            (index * size_of::<T>()) as BufferAddress,
            bytemuck::cast_slice(values),
        );
    }

    pub fn write_bytes(&mut self, queue: &Queue, offset: BufferAddress, bytes: &[u8]) {
        queue.write_buffer(&self.buffer, offset, bytes);
    }

    pub fn as_entire_binding(&self) -> wgpu::BindingResource {
        wgpu::BindingResource::Buffer(wgpu::BufferBinding {
            buffer: &self.buffer,
            offset: 0,
            size: NonZeroU64::new(self.size_bytes()),
        })
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
        (size_of::<T>() * self.len) as BufferAddress
    }

    pub fn full_slice(&self) -> BufferSlice {
        self.buffer.slice(0..self.size_bytes())
    }
}
