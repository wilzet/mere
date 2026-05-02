use crate::{gpu_buffer::GpuBuffer, handle::ResourceHandle};
use core::ops::Range;
use mere_mesh::{Meshlet, MeshletMesh, Vertex};
use std::{collections::HashMap, sync::Arc};

#[derive(Debug)]
pub struct MeshletStorage {
    pub vertices: GpuBuffer<Arc<[Vertex]>>,
    pub vertex_indices: GpuBuffer<Arc<[u32]>>,
    pub indices: GpuBuffer<Arc<[u8]>>,
    pub meshlets: GpuBuffer<Arc<[Meshlet]>>,
    meshlet_mesh_slices: HashMap<ResourceHandle<MeshletMesh>, [Range<wgpu::BufferAddress>; 4]>,
}

impl MeshletStorage {
    pub fn new(device: &wgpu::Device) -> Self {
        Self {
            vertices: GpuBuffer::new(Some("meshlet_vertices"), device),
            vertex_indices: GpuBuffer::new(Some("meshlet_vertex_indices"), device),
            indices: GpuBuffer::new(Some("meshlet_indices"), device),
            meshlets: GpuBuffer::new(Some("meshlets"), device),
            meshlet_mesh_slices: HashMap::new(),
        }
    }

    pub fn meshlet_count(&self) -> usize {
        self.meshlet_mesh_slices.len()
    }

    pub fn queue_upload(&mut self, mesh: &MeshletMesh) {
        let handle = ResourceHandle::from(mesh.name.as_str());
        self.meshlet_mesh_slices.entry(handle).or_insert_with(|| {
            let vertices_slice = self.vertices.queue_write(Arc::clone(&mesh.vertices), ());
            let vertex_indices_slice = self
                .vertex_indices
                .queue_write(Arc::clone(&mesh.meshlet_vertex_indices), ());
            let indices_slice = self
                .indices
                .queue_write(Arc::clone(&mesh.meshlet_indices), ());
            let meshlets_slice = self.meshlets.queue_write(
                Arc::clone(&mesh.meshlets),
                (vertices_slice.start, indices_slice.start),
            );

            [
                vertices_slice,
                vertex_indices_slice,
                indices_slice,
                meshlets_slice,
            ]
        });
    }

    pub fn remove(&mut self, handle: ResourceHandle<MeshletMesh>) {
        if let Some(
            [
                vertices_slice,
                vertex_indices_slice,
                indices_slice,
                meshlets_slice,
            ],
        ) = self.meshlet_mesh_slices.remove(&handle)
        {
            self.vertices.mark_slice_unused(vertices_slice);
            self.vertex_indices.mark_slice_unused(vertex_indices_slice);
            self.indices.mark_slice_unused(indices_slice);
            self.meshlets.mark_slice_unused(meshlets_slice);
        }
    }

    pub fn perform_pending_uploads(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        self.vertices.perform_writes(device, queue);
        self.vertex_indices.perform_writes(device, queue);
        self.indices.perform_writes(device, queue);
        self.meshlets.perform_writes(device, queue);
    }
}
