use crate::{camera::Camera, instance_storage::InstanceStorage, meshlet_storage::MeshletStorage};
use mere_mesh::Meshlet;
use render_resources::*;
use wgpu::util::DeviceExt;

mod render_resources;

#[derive(Clone, Debug)]
pub struct ResourceStorage {
    pub cluster_info: wgpu::Buffer,
    pub visible_cluster_info: wgpu::Buffer,
    pub main_render_view: wgpu::Buffer,
    pub render_view: wgpu::Buffer,

    pub depth_pyramid: DepthPyramid,
    pub meshlet_per_frame_resources: Option<PerFrameResources>,

    pub rightmost_slot: u32,
    pub instance_cull_bind_group_layout: wgpu::BindGroupLayout,
    pub cluster_cull_bind_group_layout: wgpu::BindGroupLayout,
    pub meshlet_mesh_material_bind_group_layout: wgpu::BindGroupLayout,
    pub render_view_bind_group_layout: wgpu::BindGroupLayout,

    pub depth_downsample_bind_group_layout: wgpu::BindGroupLayout,
}

impl ResourceStorage {
    pub fn new(
        cluster_slots: u32,
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
    ) -> Self {
        Self {
            cluster_info: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("meshlet_cluster_info"),
                size: 2 * cluster_slots as u64 * size_of::<u32>() as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            visible_cluster_info: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("meshlet_visible_cluster_info"),
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
            depth_pyramid: DepthPyramid::new(
                device,
                config,
                "depth_pyramid",
                config.width,
                config.height,
            ),
            meshlet_per_frame_resources: None,
            rightmost_slot: cluster_slots - 1,
            instance_cull_bind_group_layout: device.create_bind_group_layout(&Layout::sequential(
                Some("instance_cull_bind_group_layout"),
                wgpu::ShaderStages::COMPUTE,
                &mut [
                    storage_buffer(true),
                    storage_buffer(true),
                    storage_buffer(true),
                    storage_buffer(true),
                    storage_buffer(false),
                    storage_buffer(false),
                    storage_buffer(false),
                ],
            )),
            cluster_cull_bind_group_layout: device.create_bind_group_layout(&Layout::sequential(
                Some("cluster_cull_bind_group_layout"),
                wgpu::ShaderStages::COMPUTE,
                &mut [
                    storage_buffer(true),
                    storage_buffer(true),
                    storage_buffer(true),
                    storage_buffer(true),
                    storage_buffer(false),
                    storage_buffer(false),
                ],
            )),
            meshlet_mesh_material_bind_group_layout: device.create_bind_group_layout(
                &Layout::sequential(
                    Some("meshlet_mesh_material_bind_group_layout"),
                    wgpu::ShaderStages::VERTEX,
                    &mut [
                        storage_buffer(true),
                        storage_buffer(true),
                        storage_buffer(true),
                        storage_buffer(true),
                        storage_buffer(true),
                        storage_buffer(true),
                    ],
                ),
            ),
            render_view_bind_group_layout: device.create_bind_group_layout(&Layout::sequential(
                Some("render_view_bind_group_layout"),
                wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::COMPUTE,
                &mut [storage_buffer(true)],
            )),
            depth_downsample_bind_group_layout: device.create_bind_group_layout(
                &Layout::sequential(
                    Some("depth_downsample_bind_group_layout"),
                    wgpu::ShaderStages::COMPUTE,
                    &mut [
                        entry(wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Depth,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        }),
                        storage_texture(
                            wgpu::TextureFormat::R32Float,
                            wgpu::StorageTextureAccess::WriteOnly,
                        ),
                        storage_texture(
                            wgpu::TextureFormat::R32Float,
                            wgpu::StorageTextureAccess::WriteOnly,
                        ),
                        storage_texture(
                            wgpu::TextureFormat::R32Float,
                            wgpu::StorageTextureAccess::WriteOnly,
                        ),
                        storage_texture(
                            wgpu::TextureFormat::R32Float,
                            wgpu::StorageTextureAccess::WriteOnly,
                        ),
                        storage_texture(
                            wgpu::TextureFormat::R32Float,
                            wgpu::StorageTextureAccess::WriteOnly,
                        ),
                        storage_texture(
                            wgpu::TextureFormat::R32Float,
                            wgpu::StorageTextureAccess::ReadWrite,
                        ),
                        storage_texture(
                            wgpu::TextureFormat::R32Float,
                            wgpu::StorageTextureAccess::WriteOnly,
                        ),
                        storage_texture(
                            wgpu::TextureFormat::R32Float,
                            wgpu::StorageTextureAccess::WriteOnly,
                        ),
                        storage_texture(
                            wgpu::TextureFormat::R32Float,
                            wgpu::StorageTextureAccess::WriteOnly,
                        ),
                        storage_texture(
                            wgpu::TextureFormat::R32Float,
                            wgpu::StorageTextureAccess::WriteOnly,
                        ),
                        storage_texture(
                            wgpu::TextureFormat::R32Float,
                            wgpu::StorageTextureAccess::WriteOnly,
                        ),
                        storage_texture(
                            wgpu::TextureFormat::R32Float,
                            wgpu::StorageTextureAccess::WriteOnly,
                        ),
                        entry(wgpu::BindingType::Sampler(
                            wgpu::SamplerBindingType::NonFiltering,
                        )),
                    ],
                ),
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

        let indirect_cluster_args = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("indirect_cluster_args"),
            contents: wgpu::util::DispatchIndirectArgs { x: 0, y: 1, z: 1 }.as_bytes(),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::INDIRECT,
        });

        let indirect_draw_args = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("indirect_draw_args"),
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
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: indirect_cluster_args.as_entire_binding(),
                    },
                ],
            }),
            cluster_cull_bind_group: device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("cluster_cull_bind_group"),
                layout: &self.cluster_cull_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: instances.instance_uniforms.binding().unwrap(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: meshlets.meshlets.binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.cluster_info.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: visible_instance_cluster_count.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: self.visible_cluster_info.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: indirect_draw_args.as_entire_binding(),
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
                            resource: self.visible_cluster_info.as_entire_binding(),
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
            indirect_cluster_args,
            indirect_draw_args,
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

    pub fn resize(
        &mut self,
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        width: u32,
        height: u32,
    ) {
        self.depth_pyramid = DepthPyramid::new(
            device,
            config,
            &self.depth_pyramid.depth.label,
            width,
            height,
        )
    }
}

struct Layout;

impl Layout {
    pub fn sequential<'a, 'b: 'a, 'c: 'a>(
        label: Option<&'b str>,
        visibility: wgpu::ShaderStages,
        partials: &'c mut [wgpu::BindGroupLayoutEntry],
    ) -> wgpu::BindGroupLayoutDescriptor<'a> {
        partials.iter_mut().enumerate().for_each(|(i, e)| {
            e.binding = i as u32;
            e.visibility = visibility;
        });

        wgpu::BindGroupLayoutDescriptor {
            label,
            entries: partials,
        }
    }
}

const fn entry(ty: wgpu::BindingType) -> wgpu::BindGroupLayoutEntry {
    layout_entry(0, ty)
}

const fn storage_buffer(read_only: bool) -> wgpu::BindGroupLayoutEntry {
    storage_buffer_layout_entry(0, read_only)
}

const fn storage_texture(
    format: wgpu::TextureFormat,
    access: wgpu::StorageTextureAccess,
) -> wgpu::BindGroupLayoutEntry {
    storage_texture_layout_entry(0, format, access)
}

const fn layout_entry(binding: u32, ty: wgpu::BindingType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::empty(),
        ty,
        count: None,
    }
}

const fn storage_buffer_layout_entry(binding: u32, read_only: bool) -> wgpu::BindGroupLayoutEntry {
    layout_entry(
        binding,
        wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
    )
}

const fn storage_texture_layout_entry(
    binding: u32,
    format: wgpu::TextureFormat,
    access: wgpu::StorageTextureAccess,
) -> wgpu::BindGroupLayoutEntry {
    layout_entry(
        binding,
        wgpu::BindingType::StorageTexture {
            access,
            format,
            view_dimension: wgpu::TextureViewDimension::D2,
        },
    )
}
