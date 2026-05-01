use core::ops::Range;
use mere_mesh::{Meshlet, Vertex};
use range_alloc::RangeAllocator;
use std::{num::NonZero, sync::Arc};
use wgpu::util::DeviceExt;

#[derive(Clone, Debug)]
pub struct GpuStorageBuffer<T: GpuBufferable> {
    value: T,
    label: Option<&'static str>,
    buffer: Option<wgpu::Buffer>,
    last_written_size: u64,
}

impl<T: GpuBufferable> GpuStorageBuffer<T> {
    pub fn new(label: Option<&'static str>, value: T) -> Self {
        Self {
            value,
            label,
            buffer: None,
            last_written_size: 0,
        }
    }

    pub fn get(&self) -> &T {
        &self.value
    }

    pub fn get_mut(&mut self) -> &mut T {
        &mut self.value
    }

    pub fn write_buffer(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let data_size = self.value.size_in_bytes() as u64;
        debug_assert!(data_size.is_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT));

        let capacity = self.buffer.as_ref().map(wgpu::Buffer::size).unwrap_or(0);
        if capacity < data_size {
            self.buffer = Some(
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: self.label,
                    contents: self.value.as_bytes(),
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                }),
            );
        } else if let Some(buffer) = &self.buffer {
            queue.write_buffer(buffer, 0, self.value.as_bytes());
        }

        self.last_written_size = data_size;
    }

    pub fn binding(&self) -> Option<wgpu::BindingResource<'_>> {
        Some(wgpu::BindingResource::Buffer(wgpu::BufferBinding {
            buffer: self.buffer.as_ref()?,
            offset: 0,
            size: NonZero::new(self.last_written_size),
        }))
    }
}

#[derive(Debug)]
pub struct GpuBuffer<T: GpuBufferable> {
    label: Option<&'static str>,
    buffer: wgpu::Buffer,
    allocator: RangeAllocator<wgpu::BufferAddress>,
    write_queue: Vec<(T, T::Metadata, Range<wgpu::BufferAddress>)>,
}

impl<T: GpuBufferable> GpuBuffer<T> {
    pub fn new(label: Option<&'static str>, device: &wgpu::Device) -> Self {
        Self {
            label,
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label,
                size: 0,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            allocator: RangeAllocator::new(0..0),
            write_queue: Vec::new(),
        }
    }

    pub fn queue_write(&mut self, data: T, metadata: T::Metadata) -> Range<wgpu::BufferAddress> {
        let data_size = data.size_in_bytes() as u64;
        debug_assert!(data_size.is_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT));

        let buffer_slice = self
            .allocator
            .allocate_range(data_size)
            .unwrap_or_else(|_| {
                let buffer_size = self.allocator.initial_range();
                let double_buffer_size = (buffer_size.end - buffer_size.start) * 2;
                let new_size = double_buffer_size.max(data_size);
                self.allocator.grow_to(buffer_size.end + new_size);

                self.allocator.allocate_range(data_size).unwrap()
            });

        self.write_queue
            .push((data, metadata, buffer_slice.clone()));
        buffer_slice
    }

    pub fn perform_writes(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        if self.allocator.initial_range().end > self.buffer.size() {
            self.expand_buffer(device, queue);
        }

        let queue_count = self.write_queue.len();

        self.write_queue
            .drain(..)
            .for_each(|(data, metadata, buffer_slice)| {
                let buffer_slice_size =
                    NonZero::new(buffer_slice.end - buffer_slice.start).unwrap();
                let mut buffer_view = queue
                    .write_buffer_with(&self.buffer, buffer_slice.start, buffer_slice_size)
                    .unwrap();
                data.write_bytes_le(metadata, buffer_view.slice(..), buffer_slice.start);
            });

        let queue_cap = self.write_queue.capacity();
        let queue_saturation = queue_count as f32 / queue_cap as f32;
        if queue_saturation < 0.3 {
            self.write_queue.shrink_to(queue_cap / 4);
        }
    }

    pub fn mark_slice_unused(&mut self, buffer_slice: Range<wgpu::BufferAddress>) {
        self.allocator.free_range(buffer_slice);
    }

    pub fn binding(&self) -> wgpu::BindingResource<'_> {
        self.buffer.as_entire_binding()
    }

    fn expand_buffer(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let size = self.allocator.initial_range();
        let new_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: self.label,
            size: size.end - size.start,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("expand_gpu_buffer"),
        });
        encoder.copy_buffer_to_buffer(&self.buffer, 0, &new_buffer, 0, self.buffer.size());
        queue.submit(Some(encoder.finish()));

        self.buffer = new_buffer;
    }
}

pub trait GpuBufferable {
    type Metadata;

    fn size_in_bytes(&self) -> usize;
    fn as_bytes(&self) -> &[u8];
    fn write_bytes_le(
        &self,
        _metadata: Self::Metadata,
        mut buffer_slice: wgpu::WriteOnly<[u8]>,
        _buffer_offset: wgpu::BufferAddress,
    ) {
        buffer_slice.copy_from_slice(self.as_bytes());
    }
}

impl GpuBufferable for Arc<[Meshlet]> {
    type Metadata = (wgpu::BufferAddress, wgpu::BufferAddress);

    fn size_in_bytes(&self) -> usize {
        self.len() * size_of::<Meshlet>()
    }

    fn as_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(self)
    }

    fn write_bytes_le(
        &self,
        (vertex_offset, index_offset): Self::Metadata,
        mut buffer_slice: wgpu::WriteOnly<[u8]>,
        _buffer_offset: wgpu::BufferAddress,
    ) {
        self.iter().enumerate().for_each(|(i, meshlet)| {
            const DATA_SIZE: usize = size_of::<Meshlet>();
            let i = i * DATA_SIZE;
            let bytes = bytemuck::cast::<_, [u8; DATA_SIZE]>(Meshlet {
                vertex_offset: meshlet.vertex_offset + vertex_offset as u32,
                index_offset: meshlet.index_offset + index_offset as u32,
                ..*meshlet
            });
            buffer_slice
                .slice(i..(i + DATA_SIZE))
                .copy_from_slice(&bytes);
        });
    }
}

impl GpuBufferable for Arc<[Vertex]> {
    type Metadata = ();

    fn size_in_bytes(&self) -> usize {
        self.len() * size_of::<Vertex>()
    }

    fn as_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(self)
    }
}

impl GpuBufferable for Arc<[u8]> {
    type Metadata = ();

    fn size_in_bytes(&self) -> usize {
        self.len()
    }

    fn as_bytes(&self) -> &[u8] {
        self
    }
}

impl GpuBufferable for Arc<[u32]> {
    type Metadata = ();

    fn size_in_bytes(&self) -> usize {
        self.len() * size_of::<u32>()
    }

    fn as_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(self)
    }
}

impl<T: bytemuck::Pod + bytemuck::Zeroable> GpuBufferable for Vec<T> {
    type Metadata = ();

    fn size_in_bytes(&self) -> usize {
        self.len() * size_of::<T>()
    }

    fn as_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(self)
    }
}

// Code for buffers based on:
//  * https://github.com/bevyengine/bevy/blob/c3c118c20c984db1cbba5acf1e6042bca5f7bba8/crates/bevy_pbr/src/meshlet/persistent_buffer.rs
//  * https://github.com/bevyengine/bevy/blob/251bcacf6bab128c1d5fc79f3436e807a614208d/crates/bevy_render/src/render_resource/storage_buffer.rs
//
// Edited by permission of the MIT license:
//
// MIT License
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.
