use crate::{instance_storage::InstanceStorage, meshlet_storage::MeshletStorage};
use mere_mesh::Meshlet;
use wgpu::util::DeviceExt;

#[derive(Clone, Debug)]
pub struct ResourceStorage {
    pub cluster_info: wgpu::Buffer,

    pub rightmost_slot: u32,
    pub instance_cull_bind_group_layout: wgpu::BindGroupLayout,
    pub cluster_cull_bind_group_layout: wgpu::BindGroupLayout,
    pub meshlet_mesh_material_bind_group_layout: wgpu::BindGroupLayout,
}

impl ResourceStorage {
    pub fn new(cluster_slots: u32, device: &wgpu::Device) -> Self {
        Self {
            cluster_info: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("meshlet_cluster_info"),
                size: 2 * cluster_slots as u64 * size_of::<u32>() as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            rightmost_slot: cluster_slots - 1,
            instance_cull_bind_group_layout: device.create_bind_group_layout(
                &wgpu::BindGroupLayoutDescriptor {
                    label: Some("instance_cull_bind_group_layout"),
                    entries: &[
                        storage_buffer_layout_entry(0, wgpu::ShaderStages::COMPUTE, true),
                        storage_buffer_layout_entry(1, wgpu::ShaderStages::COMPUTE, true),
                        storage_buffer_layout_entry(2, wgpu::ShaderStages::COMPUTE, true),
                        storage_buffer_layout_entry(3, wgpu::ShaderStages::COMPUTE, false),
                        storage_buffer_layout_entry(4, wgpu::ShaderStages::COMPUTE, false),
                    ],
                },
            ),
            cluster_cull_bind_group_layout: device.create_bind_group_layout(
                &wgpu::BindGroupLayoutDescriptor {
                    label: Some("cluster_cull_bind_group_layout"),
                    entries: &[
                        storage_buffer_layout_entry(0, wgpu::ShaderStages::COMPUTE, true),
                        storage_buffer_layout_entry(1, wgpu::ShaderStages::COMPUTE, true),
                        storage_buffer_layout_entry(2, wgpu::ShaderStages::COMPUTE, false),
                    ],
                },
            ),
            meshlet_mesh_material_bind_group_layout: device.create_bind_group_layout(
                &wgpu::BindGroupLayoutDescriptor {
                    label: Some("meshlet_mesh_material_bind_group_layout"),
                    entries: &[
                        storage_buffer_layout_entry(0, wgpu::ShaderStages::VERTEX, true),
                        storage_buffer_layout_entry(1, wgpu::ShaderStages::VERTEX, true),
                        storage_buffer_layout_entry(2, wgpu::ShaderStages::VERTEX, true),
                        storage_buffer_layout_entry(3, wgpu::ShaderStages::VERTEX, true),
                        storage_buffer_layout_entry(4, wgpu::ShaderStages::VERTEX, true),
                        storage_buffer_layout_entry(5, wgpu::ShaderStages::VERTEX, true),
                    ],
                },
            ),
        }
    }

    pub fn bind_groups(
        &self,
        device: &wgpu::Device,
        meshlets: &MeshletStorage,
        instances: &InstanceStorage,
    ) -> (MeshletBindGroups, PerFrameResources) {
        let per_frame_resources = PerFrameResources {
            visible_instance_cluster_count: device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("visible_instance_cluster_count"),
                    contents: bytemuck::bytes_of(&0u32),
                    usage: wgpu::BufferUsages::STORAGE,
                },
            ),
            indirect_args: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("indirect_args"),
                contents: wgpu::util::DrawIndirectArgs {
                    vertex_count: 3 * Meshlet::MAX_VERTICES as u32,
                    instance_count: 0,
                    first_vertex: 0,
                    first_instance: 0,
                }
                .as_bytes(),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::INDIRECT,
            }),
        };

        (
            MeshletBindGroups {
                instance_cull_bind_group: device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("instance_cull_bind_group"),
                    layout: &self.instance_cull_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: instances.instance_aabbs.binding().unwrap(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: instances.instance_meshlet_offsets.binding().unwrap(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: instances.instance_meshlet_counts.binding().unwrap(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: self.cluster_info.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 4,
                            resource: per_frame_resources
                                .visible_instance_cluster_count
                                .as_entire_binding(),
                        },
                    ],
                }),
                cluster_cull_bind_group: device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("cluster_cull_bind_group"),
                    layout: &self.cluster_cull_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: self.cluster_info.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: meshlets.meshlets.binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: per_frame_resources.indirect_args.as_entire_binding(),
                        },
                    ],
                }),
                meshlet_mesh_material_bind_group: device.create_bind_group(
                    &wgpu::BindGroupDescriptor {
                        label: Some("meshlet_mesh_material_bind_group"),
                        layout: &self.meshlet_mesh_material_bind_group_layout,
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
                            wgpu::BindGroupEntry {
                                binding: 5,
                                resource: self.cluster_info.as_entire_binding(),
                            },
                        ],
                    },
                ),
            },
            per_frame_resources,
        )
    }
}

const fn storage_buffer(read_only: bool) -> wgpu::BindingType {
    wgpu::BindingType::Buffer {
        ty: wgpu::BufferBindingType::Storage { read_only },
        has_dynamic_offset: false,
        min_binding_size: None,
    }
}

const fn storage_buffer_layout_entry(
    binding: u32,
    visibility: wgpu::ShaderStages,
    read_only: bool,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: storage_buffer(read_only),
        count: None,
    }
}

pub struct MeshletBindGroups {
    pub instance_cull_bind_group: wgpu::BindGroup,
    pub cluster_cull_bind_group: wgpu::BindGroup,
    pub meshlet_mesh_material_bind_group: wgpu::BindGroup,
}

pub struct PerFrameResources {
    pub visible_instance_cluster_count: wgpu::Buffer,
    pub indirect_args: wgpu::Buffer,
}
