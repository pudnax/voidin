use std::{borrow::Cow, marker::PhantomData, mem::size_of};

use wgpu::{
    util::DeviceExt, Buffer, BufferDescriptor, BufferSlice, BufferUsages, CommandEncoderDescriptor,
    Device, Queue,
};

pub struct ResizingBuffer<T> {
    label: Cow<'static, str>,
    buffer: Buffer,
    usages: BufferUsages,
    len: usize,
    cap: usize,
    _phantom: PhantomData<T>,
}

impl<T> std::ops::Deref for ResizingBuffer<T> {
    type Target = Buffer;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

impl<T: bytemuck::Pod> ResizingBuffer<T> {
    pub fn new(device: &Device, usages: BufferUsages, label: Cow<'static, str>) -> Self {
        let default_cap = 32;
        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some(&label),
            size: (size_of::<T>() * default_cap) as u64,

            usage: usages | BufferUsages::COPY_SRC | BufferUsages::COPY_DST,

            mapped_at_creation: false,
        });

        Self {
            buffer,

            label,
            usages,
            len: 0,
            cap: default_cap,
            _phantom: PhantomData,
        }
    }

    pub fn new_with_data(
        device: &Device,
        usages: BufferUsages,
        label: Cow<'static, str>,
        data: &[T],
    ) -> Self {
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&label),
            contents: bytemuck::cast_slice(data),
            usage: usages | BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
        });

        Self {
            buffer,

            label,
            usages,
            len: data.len(),
            cap: data.len(),
            _phantom: PhantomData,
        }
    }

    pub fn extend(&mut self, device: &Device, queue: &Queue, values: &[T]) {
        let new_len = self.len() + values.len();

        if new_len >= self.cap {
            let new_cap = new_len.next_power_of_two();
            let new_buf = device.create_buffer(&BufferDescriptor {
                label: Some(&self.label),
                size: (size_of::<T>() * new_cap) as u64,
                usage: self.usages,
                mapped_at_creation: false,
            });

            let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
                label: Some(&self.label),
            });

            encoder.copy_buffer_to_buffer(&self.buffer, 0, &new_buf, 0, self.size_bytes());
            queue.submit(Some(encoder.finish()));

            self.cap = new_cap;
            self.buffer = new_buf;
        }

        queue.write_buffer(
            &self.buffer,
            self.size_bytes(),
            bytemuck::cast_slice(values),
        );
        self.len = new_len;
    }

    pub fn usages(&self) -> BufferUsages {
        self.usages
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }

    pub fn size_bytes(&self) -> u64 {
        (size_of::<T>() * self.len) as u64
    }

    pub fn full_slice(&self) -> BufferSlice {
        self.buffer.slice(0..self.size_bytes())
    }
}
