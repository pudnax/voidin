use std::{borrow::Cow, marker::PhantomData, mem::size_of};

use wgpu::{
    util::DeviceExt, Buffer, BufferDescriptor, BufferSlice, BufferUsages, CommandEncoderDescriptor,
    Device, Queue,
};

pub struct ResizableBuffer<T> {
    label: Cow<'static, str>,
    buffer: Buffer,
    usages: BufferUsages,
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
    pub fn new(device: &Device, usages: BufferUsages, label: Option<Cow<'static, str>>) -> Self {
        let default_cap = 32;
        let label = label.unwrap_or("Resizable Buffer".into());
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
        label: Option<Cow<'static, str>>,
        data: &[T],
    ) -> Self {
        let label = label.unwrap_or("Resizable Buffer".into());
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

    pub fn push(&mut self, device: &Device, queue: &Queue, values: &[T]) -> bool {
        let new_len = self.len() + values.len();
        let mut was_reallocated = false;

        if new_len >= self.cap {
            let max_buffer_size = device.limits().max_buffer_size;
            let new_cap = new_len
                .checked_next_power_of_two()
                .unwrap_or(new_len)
                .min(max_buffer_size as usize / size_of::<T>());
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
            was_reallocated = true;
        }

        queue.write_buffer(
            &self.buffer,
            self.size_bytes(),
            bytemuck::cast_slice(values),
        );
        self.len = new_len;
        was_reallocated
    }

    pub fn write(&mut self, queue: &Queue, offset: u64, value: T) {
        assert!(size_of::<T>() as u64 + offset <= self.size_bytes());
        queue.write_buffer(&self.buffer, offset, bytemuck::bytes_of(&value));
    }

    pub fn write_slice(&mut self, queue: &Queue, offset: u64, values: &[T]) {
        assert!((values.len() * size_of::<T>()) as u64 + offset <= self.size_bytes());
        queue.write_buffer(&self.buffer, offset, bytemuck::cast_slice(values));
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
