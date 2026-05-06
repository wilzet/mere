use crate::{Camera, instance_storage::InstanceStorage, meshlet_storage::MeshletStorage};
use mere_math::Vec4Swizzles;
use mere_mesh::Meshlet;
use wgpu::util::DeviceExt;

#[derive(Clone, Debug)]
pub struct ResourceStorage {
    pub cluster_info: wgpu::Buffer,
    pub main_render_view: wgpu::Buffer,
    pub render_view: wgpu::Buffer,

    pub meshlet_per_frame_resources: Option<PerFrameResources>,

    pub rightmost_slot: u32,
    pub instance_cull_bind_group_layout: wgpu::BindGroupLayout,
    pub cluster_cull_bind_group_layout: wgpu::BindGroupLayout,
    pub meshlet_mesh_material_bind_group_layout: wgpu::BindGroupLayout,
    pub render_view_bind_group_layout: wgpu::BindGroupLayout,
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
            main_render_view: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("main_render_view"),
                size: size_of::<RenderView>() as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            render_view: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("render_view"),
                size: size_of::<RenderView>() as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            meshlet_per_frame_resources: None,
            rightmost_slot: cluster_slots - 1,
            instance_cull_bind_group_layout: device.create_bind_group_layout(
                &wgpu::BindGroupLayoutDescriptor {
                    label: Some("instance_cull_bind_group_layout"),
                    entries: &[
                        storage_buffer_layout_entry(0, wgpu::ShaderStages::COMPUTE, true),
                        storage_buffer_layout_entry(1, wgpu::ShaderStages::COMPUTE, true),
                        storage_buffer_layout_entry(2, wgpu::ShaderStages::COMPUTE, true),
                        storage_buffer_layout_entry(3, wgpu::ShaderStages::COMPUTE, true),
                        storage_buffer_layout_entry(4, wgpu::ShaderStages::COMPUTE, false),
                        storage_buffer_layout_entry(5, wgpu::ShaderStages::COMPUTE, false),
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
            render_view_bind_group_layout: device.create_bind_group_layout(
                &wgpu::BindGroupLayoutDescriptor {
                    label: Some("render_view_bind_group_layout"),
                    entries: &[storage_buffer_layout_entry(
                        0,
                        wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::COMPUTE,
                        true,
                    )],
                },
            ),
        }
    }

    pub fn generate_frame_resources(
        &mut self,
        device: &wgpu::Device,
        meshlets: &MeshletStorage,
        instances: &InstanceStorage,
    ) {
        let visible_instance_cluster_count =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("visible_instance_cluster_count"),
                contents: bytemuck::bytes_of(&0u32),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let indirect_args = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("indirect_args"),
            contents: wgpu::util::DrawIndirectArgs {
                vertex_count: Meshlet::MAX_INDICES_PER_MESHLET,
                instance_count: 0,
                first_vertex: 0,
                first_instance: 0,
            }
            .as_bytes(),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::INDIRECT,
        });

        let bind_groups = MeshletBindGroups {
            instance_cull_bind_group: device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("instance_cull_bind_group"),
                layout: &self.instance_cull_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: instances.instance_uniforms.binding().unwrap(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: instances.instance_aabbs.binding().unwrap(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: instances.instance_meshlet_offsets.binding().unwrap(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: instances.instance_meshlet_counts.binding().unwrap(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: self.cluster_info.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: visible_instance_cluster_count.as_entire_binding(),
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
                        resource: indirect_args.as_entire_binding(),
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
            main_render_view_bind_group: device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("main_render_view_bind_group"),
                layout: &self.render_view_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.main_render_view.as_entire_binding(),
                }],
            }),
            render_view_bind_group: device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("render_view_bind_group"),
                layout: &self.render_view_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.render_view.as_entire_binding(),
                }],
            }),
        };

        self.meshlet_per_frame_resources = Some(PerFrameResources {
            visible_instance_cluster_count,
            indirect_args,
            bind_groups,
        });
    }

    pub fn update_render_view(&self, queue: &wgpu::Queue, camera: &Camera, update_view: bool) {
        queue.write_buffer(
            &self.main_render_view,
            0,
            bytemuck::cast_slice(&[RenderView::from_camera(camera)]),
        );

        if update_view {
            queue.write_buffer(
                &self.render_view,
                0,
                bytemuck::cast_slice(&[RenderView::from_camera(camera)]),
            );
        }
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
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

#[derive(Clone, Debug)]
pub struct MeshletBindGroups {
    pub instance_cull_bind_group: wgpu::BindGroup,
    pub cluster_cull_bind_group: wgpu::BindGroup,
    pub meshlet_mesh_material_bind_group: wgpu::BindGroup,
    pub main_render_view_bind_group: wgpu::BindGroup,
    pub render_view_bind_group: wgpu::BindGroup,
}

#[derive(Clone, Debug)]
pub struct PerFrameResources {
    pub visible_instance_cluster_count: wgpu::Buffer,
    pub indirect_args: wgpu::Buffer,
    pub bind_groups: MeshletBindGroups,
}

#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Debug)]
#[repr(C)]
pub struct RenderView {
    view_position: [f32; 4],
    view_proj: [[f32; 4]; 4],
    frustum: [[f32; 4]; 6],
}

impl RenderView {
    pub fn from_camera(camera: &Camera) -> Self {
        let view_proj = camera.projection_matrix() * camera.view_matrix();

        let row = |i: usize| view_proj.row(i);

        let mut planes = [
            (row(3) + row(0)), // Left
            (row(3) - row(0)), // Right
            (row(3) + row(1)), // Bottom
            (row(3) - row(1)), // Top
            (row(3) + row(2)), // Near
            (row(3) - row(2)), // Far
        ];

        for plane in planes.iter_mut() {
            let length = plane.xyz().length();
            *plane /= length;
        }

        Self {
            view_position: camera.transform.translation.to_homogeneous().into(),
            view_proj: view_proj.to_cols_array_2d(),
            frustum: planes.map(|p| p.to_array()),
        }
    }
}
