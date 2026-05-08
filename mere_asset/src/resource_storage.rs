use crate::{
    Camera, Texture, instance_storage::InstanceStorage, meshlet_storage::MeshletStorage,
    texture::TextureOptions,
};
use mere_math::Vec4Swizzles;
use mere_mesh::Meshlet;
use wgpu::util::DeviceExt;

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
                        storage_buffer_layout_entry(6, wgpu::ShaderStages::COMPUTE, false),
                    ],
                },
            ),
            cluster_cull_bind_group_layout: device.create_bind_group_layout(
                &wgpu::BindGroupLayoutDescriptor {
                    label: Some("cluster_cull_bind_group_layout"),
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
            depth_downsample_bind_group_layout: device.create_bind_group_layout(
                &wgpu::BindGroupLayoutDescriptor {
                    label: Some("depth_downsample_bind_group_layout"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Depth,
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        storage_texture_layout_entry(
                            1,
                            wgpu::ShaderStages::COMPUTE,
                            wgpu::TextureFormat::R32Float,
                            wgpu::StorageTextureAccess::WriteOnly,
                        ),
                        storage_texture_layout_entry(
                            2,
                            wgpu::ShaderStages::COMPUTE,
                            wgpu::TextureFormat::R32Float,
                            wgpu::StorageTextureAccess::WriteOnly,
                        ),
                        storage_texture_layout_entry(
                            3,
                            wgpu::ShaderStages::COMPUTE,
                            wgpu::TextureFormat::R32Float,
                            wgpu::StorageTextureAccess::WriteOnly,
                        ),
                        storage_texture_layout_entry(
                            4,
                            wgpu::ShaderStages::COMPUTE,
                            wgpu::TextureFormat::R32Float,
                            wgpu::StorageTextureAccess::WriteOnly,
                        ),
                        storage_texture_layout_entry(
                            5,
                            wgpu::ShaderStages::COMPUTE,
                            wgpu::TextureFormat::R32Float,
                            wgpu::StorageTextureAccess::WriteOnly,
                        ),
                        storage_texture_layout_entry(
                            6,
                            wgpu::ShaderStages::COMPUTE,
                            wgpu::TextureFormat::R32Float,
                            wgpu::StorageTextureAccess::ReadWrite,
                        ),
                        storage_texture_layout_entry(
                            7,
                            wgpu::ShaderStages::COMPUTE,
                            wgpu::TextureFormat::R32Float,
                            wgpu::StorageTextureAccess::WriteOnly,
                        ),
                        storage_texture_layout_entry(
                            8,
                            wgpu::ShaderStages::COMPUTE,
                            wgpu::TextureFormat::R32Float,
                            wgpu::StorageTextureAccess::WriteOnly,
                        ),
                        storage_texture_layout_entry(
                            9,
                            wgpu::ShaderStages::COMPUTE,
                            wgpu::TextureFormat::R32Float,
                            wgpu::StorageTextureAccess::WriteOnly,
                        ),
                        storage_texture_layout_entry(
                            10,
                            wgpu::ShaderStages::COMPUTE,
                            wgpu::TextureFormat::R32Float,
                            wgpu::StorageTextureAccess::WriteOnly,
                        ),
                        storage_texture_layout_entry(
                            11,
                            wgpu::ShaderStages::COMPUTE,
                            wgpu::TextureFormat::R32Float,
                            wgpu::StorageTextureAccess::WriteOnly,
                        ),
                        storage_texture_layout_entry(
                            12,
                            wgpu::ShaderStages::COMPUTE,
                            wgpu::TextureFormat::R32Float,
                            wgpu::StorageTextureAccess::WriteOnly,
                        ),
                        wgpu::BindGroupLayoutEntry {
                            binding: 13,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                            count: None,
                        },
                    ],
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

const fn storage_texture_layout_entry(
    binding: u32,
    visibility: wgpu::ShaderStages,
    format: wgpu::TextureFormat,
    access: wgpu::StorageTextureAccess,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::StorageTexture {
            access,
            format,
            view_dimension: wgpu::TextureViewDimension::D2,
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
    pub indirect_cluster_args: wgpu::Buffer,
    pub indirect_draw_args: wgpu::Buffer,
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

const DEPTH_PYRAMID_COUNT: usize = 12;

#[derive(Clone, Debug)]
pub struct DepthPyramid {
    pub depth: Texture,
    pub depth_pyramid: Texture,
    pub depth_pyramid_mips: [Option<wgpu::TextureView>; DEPTH_PYRAMID_COUNT],
    pub mip_count: u32,
}

impl DepthPyramid {
    pub fn new(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        label: &str,
        width: u32,
        height: u32,
    ) -> Self {
        let size = wgpu::Extent3d {
            width: width.next_power_of_two() / 2,
            height: height.next_power_of_two() / 2,
            depth_or_array_layers: 1,
        };

        let mip_count = size.max_mips(wgpu::TextureDimension::D2);

        let depth_pyramid = Texture::create_texture(
            label,
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size,
                mip_level_count: mip_count,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R32Float,
                usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            }),
            TextureOptions::default(),
            device,
        );

        let depth_pyramid_mips = std::array::from_fn(|i| {
            if (i as u32) < mip_count {
                Some(
                    depth_pyramid
                        .texture()
                        .create_view(&wgpu::TextureViewDescriptor {
                            label: Some(label),
                            format: Some(wgpu::TextureFormat::R32Float),
                            dimension: Some(wgpu::TextureViewDimension::D2),
                            usage: None,
                            aspect: wgpu::TextureAspect::All,
                            base_mip_level: i as u32,
                            mip_level_count: Some(1),
                            base_array_layer: 0,
                            array_layer_count: Some(1),
                        }),
                )
            } else {
                None
            }
        });

        Self {
            depth: Texture::create_depth_texture(&device, &config, "depth_texture"),
            depth_pyramid,
            depth_pyramid_mips,
            mip_count,
        }
    }
}
