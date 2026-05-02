use crate::{instance_storage::InstanceStorage, meshlet_storage::MeshletStorage};

#[derive(Clone, Debug)]
pub struct ResourceStorage {
    pub rightmost_slot: u32,
    pub meshlet_mesh_material_bind_group_layout: wgpu::BindGroupLayoutDescriptor<'static>,
}

impl ResourceStorage {
    pub fn new(cluster_slots: u32) -> Self {
        Self {
            rightmost_slot: cluster_slots - 1,
            meshlet_mesh_material_bind_group_layout: wgpu::BindGroupLayoutDescriptor {
                label: Some("meshlet_mesh_material_bind_group_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            },
        }
    }

    pub fn bind_groups(
        &self,
        device: &wgpu::Device,
        meshlets: &MeshletStorage,
        instances: &InstanceStorage,
    ) -> MeshletBindGroups {
        MeshletBindGroups {
            meshlet_mesh_material_bind_group: device.create_bind_group(
                &wgpu::BindGroupDescriptor {
                    label: Some("meshlet_mesh_material_bind_group"),
                    layout: &device
                        .create_bind_group_layout(&self.meshlet_mesh_material_bind_group_layout),
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: meshlets.vertices.binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: meshlets.vertex_indices.binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: meshlets.indices.binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: meshlets.meshlets.binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 4,
                            resource: instances.instance_uniforms.binding().unwrap(),
                        },
                    ],
                },
            ),
        }
    }
}

pub struct MeshletBindGroups {
    pub meshlet_mesh_material_bind_group: wgpu::BindGroup,
}
