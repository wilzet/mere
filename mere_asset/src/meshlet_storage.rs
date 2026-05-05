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

    pub fn queue_upload(&mut self, mesh: &mut MeshletMesh) -> u32 {
        let handle = ResourceHandle::from(mesh.name.as_str());
        self.meshlet_mesh_slices.entry(handle).or_insert_with(|| {
            let vertices_slice = self.vertices.queue_write(Arc::clone(&mesh.vertices), ());

            let adjusted_vertex_indices = mesh
                .meshlet_vertex_indices
                .iter()
                .map(|i| i + (vertices_slice.start / size_of::<Vertex>() as u64) as u32)
                .collect::<Vec<_>>()
                .into();
            let vertex_indices_slice = self.vertex_indices.queue_write(adjusted_vertex_indices, ());
            let indices_slice = self
                .indices
                .queue_write(Arc::clone(&mesh.meshlet_indices), ());
            let meshlets_slice = self.meshlets.queue_write(
                Arc::clone(&mesh.meshlets),
                (
                    vertex_indices_slice.start / size_of::<u32>() as u64,
                    indices_slice.start,
                ),
            );

            mesh.meshlet_offset = (meshlets_slice.start / size_of::<Meshlet>() as u64) as u32;

            [
                vertices_slice,
                vertex_indices_slice,
                indices_slice,
                meshlets_slice,
            ]
        });

        mesh.meshlet_offset
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

    pub fn report_memory(&self) -> usize {
        let mut total = 0;

        total += self.vertices.size_in_bytes();
        total += self.vertex_indices.size_in_bytes();
        total += self.indices.size_in_bytes();
        total += self.meshlets.size_in_bytes();
        total
    }
}
