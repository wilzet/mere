use crate::{
    camera::Camera, instance_storage::InstanceStorage, meshlet_storage::MeshletStorage,
    texture::Texture,
};
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

    pub visibility_buffer: wgpu::Texture,
    pub material_depth: Texture,
    pub depth_pyramid: DepthPyramid,
    pub meshlet_per_frame_resources: Option<PerFrameResources>,

    pub rightmost_slot: u32,
    pub visibility_buffer_clear_bind_group_layout: wgpu::BindGroupLayout,
    pub instance_cull_bind_group_layout: wgpu::BindGroupLayout,
    pub cluster_cull_bind_group_layout: wgpu::BindGroupLayout,
    pub visibility_buffer_raster_bind_group_layout: wgpu::BindGroupLayout,
    pub meshlet_read_attributes_bind_group_layout: wgpu::BindGroupLayout,
    pub render_view_bind_group_layout: wgpu::BindGroupLayout,
    pub resolve_material_depth_bind_group_layout: wgpu::BindGroupLayout,
    pub downsample_depth_bind_group_layout: wgpu::BindGroupLayout,

    pub debug_bind_group_layout: wgpu::BindGroupLayout,
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
            visibility_buffer: {
                let size = wgpu::Extent3d {
                    width: config.width,
                    height: config.height,
                    depth_or_array_layers: 1,
                };

                device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("visibility_buffer"),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::R64Uint,
                    usage: wgpu::TextureUsages::STORAGE_ATOMIC
                        | wgpu::TextureUsages::STORAGE_BINDING,
                    view_formats: &[],
                })
            },
            material_depth: Texture::create_depth_texture(
                device,
                config,
                "material_depth_texture",
                true,
            ),
            depth_pyramid: DepthPyramid::new(device, "depth_pyramid", config.width, config.height),
            meshlet_per_frame_resources: None,
            rightmost_slot: cluster_slots - 1,
            visibility_buffer_clear_bind_group_layout: device.create_bind_group_layout(
                &Layout::sequential(
                    Some("visibility_buffer_clear_bind_group_layout"),
                    wgpu::ShaderStages::COMPUTE,
                    &mut [storage_texture(
                        wgpu::TextureFormat::R64Uint,
                        wgpu::StorageTextureAccess::WriteOnly,
                    )],
                )
                .get(),
            ),
            instance_cull_bind_group_layout: device.create_bind_group_layout(
                &Layout::sequential(
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
                )
                .get(),
            ),
            cluster_cull_bind_group_layout: device.create_bind_group_layout(
                &Layout::sequential(
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
                )
                .get(),
            ),
            visibility_buffer_raster_bind_group_layout: device.create_bind_group_layout(
                &Layout::sequential(
                    Some("visibility_buffer_raster_bind_group_layout"),
                    wgpu::ShaderStages::VERTEX,
                    &mut [
                        storage_buffer(true),
                        storage_buffer(true),
                        storage_buffer(true),
                        storage_buffer(true),
                        storage_buffer(true),
                        storage_buffer(true),
                    ],
                )
                .with({
                    let mut entry = storage_texture(
                        wgpu::TextureFormat::R64Uint,
                        wgpu::StorageTextureAccess::Atomic,
                    );
                    entry.visibility = wgpu::ShaderStages::FRAGMENT;
                    entry
                })
                .get(),
            ),
            meshlet_read_attributes_bind_group_layout: device.create_bind_group_layout(
                &Layout::sequential(
                    Some("meshlet_read_attributes_bind_group_layout"),
                    wgpu::ShaderStages::FRAGMENT,
                    &mut [
                        storage_buffer(true),
                        storage_buffer(true),
                        storage_buffer(true),
                        storage_buffer(true),
                        storage_buffer(true),
                        storage_buffer(true),
                        storage_buffer(true),
                        storage_texture(
                            wgpu::TextureFormat::R64Uint,
                            wgpu::StorageTextureAccess::ReadOnly,
                        ),
                    ],
                )
                .get(),
            ),
            render_view_bind_group_layout: device.create_bind_group_layout(
                &Layout::sequential(
                    Some("render_view_bind_group_layout"),
                    wgpu::ShaderStages::VERTEX_FRAGMENT | wgpu::ShaderStages::COMPUTE,
                    &mut [storage_buffer(true)],
                )
                .get(),
            ),
            downsample_depth_bind_group_layout: device.create_bind_group_layout(
                &Layout::sequential(
                    Some("downsample_depth_bind_group_layout"),
                    wgpu::ShaderStages::COMPUTE,
                    &mut [
                        storage_texture(
                            wgpu::TextureFormat::R64Uint,
                            wgpu::StorageTextureAccess::ReadOnly,
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
                )
                .get(),
            ),
            resolve_material_depth_bind_group_layout: device.create_bind_group_layout(
                &Layout::sequential(
                    Some("resolve_material_depth_bind_group_layout"),
                    wgpu::ShaderStages::FRAGMENT,
                    &mut [
                        storage_buffer(true),
                        storage_buffer(true),
                        storage_texture(
                            wgpu::TextureFormat::R64Uint,
                            wgpu::StorageTextureAccess::ReadOnly,
                        ),
                    ],
                )
                .get(),
            ),
            debug_bind_group_layout: device.create_bind_group_layout(
                &wgpu::BindGroupLayoutDescriptor {
                    label: Some("debug_bind_group_layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                },
            ),
        }
    }

    pub fn generate_frame_resources(
        &mut self,
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        meshlets: &MeshletStorage,
        instances: &InstanceStorage,
    ) {
        let size = wgpu::Extent3d {
            width: config.width,
            height: config.height,
            depth_or_array_layers: 1,
        };

        let visibility_buffer = self
            .visibility_buffer
            .create_view(&wgpu::TextureViewDescriptor::default());

        let dummy_render_target = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("dummy_render_target"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Uint,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default());

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
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::INDIRECT
                | wgpu::BufferUsages::COPY_SRC,
        });

        let bind_groups = MeshletBindGroups {
            visibility_buffer_clear_bind_group: device.create_bind_group(
                &wgpu::BindGroupDescriptor {
                    label: Some("visibility_buffer_clear_bind_group"),
                    layout: &self.visibility_buffer_clear_bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&visibility_buffer),
                    }],
                },
            ),
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
            visibility_buffer_raster_bind_group: device.create_bind_group(
                &wgpu::BindGroupDescriptor {
                    label: Some("visibility_buffer_raster_bind_group"),
                    layout: &self.visibility_buffer_raster_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: meshlets.vertex_positions.binding(),
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
                        wgpu::BindGroupEntry {
                            binding: 6,
                            resource: wgpu::BindingResource::TextureView(&visibility_buffer),
                        },
                    ],
                },
            ),
            meshlet_read_attributes_bind_group: device.create_bind_group(
                &wgpu::BindGroupDescriptor {
                    label: Some("meshlet_read_attributes_bind_group"),
                    layout: &self.meshlet_read_attributes_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: meshlets.vertex_positions.binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: meshlets.vertex_attributes.binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: meshlets.vertex_indices.binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: meshlets.indices.binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 4,
                            resource: meshlets.meshlets.binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 5,
                            resource: instances.instance_uniforms.binding().unwrap(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 6,
                            resource: self.visible_cluster_info.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 7,
                            resource: wgpu::BindingResource::TextureView(&visibility_buffer),
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
            downsample_depth_bind_group: device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("downsample_depth_bind_group"),
                layout: &self.downsample_depth_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&visibility_buffer),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(
                            &self.depth_pyramid.depth_pyramid_mips[0],
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(
                            &self.depth_pyramid.depth_pyramid_mips[1],
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(
                            &self.depth_pyramid.depth_pyramid_mips[2],
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(
                            &self.depth_pyramid.depth_pyramid_mips[3],
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::TextureView(
                            &self.depth_pyramid.depth_pyramid_mips[4],
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: wgpu::BindingResource::TextureView(
                            &self.depth_pyramid.depth_pyramid_mips[5],
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 7,
                        resource: wgpu::BindingResource::TextureView(
                            &self.depth_pyramid.depth_pyramid_mips[6],
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 8,
                        resource: wgpu::BindingResource::TextureView(
                            &self.depth_pyramid.depth_pyramid_mips[7],
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 9,
                        resource: wgpu::BindingResource::TextureView(
                            &self.depth_pyramid.depth_pyramid_mips[8],
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 10,
                        resource: wgpu::BindingResource::TextureView(
                            &self.depth_pyramid.depth_pyramid_mips[9],
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 11,
                        resource: wgpu::BindingResource::TextureView(
                            &self.depth_pyramid.depth_pyramid_mips[10],
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 12,
                        resource: wgpu::BindingResource::TextureView(
                            &self.depth_pyramid.depth_pyramid_mips[11],
                        ),
                    },
                    self.depth_pyramid
                        .depth_pyramid
                        .bind_group_entry_sampler(13),
                ],
            }),
            resolve_material_depth_bind_group: device.create_bind_group(
                &wgpu::BindGroupDescriptor {
                    label: Some("resolve_material_depth_bind_group"),
                    layout: &self.resolve_material_depth_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: self.visible_cluster_info.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: instances.instance_material_ids.binding().unwrap(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::TextureView(&visibility_buffer),
                        },
                    ],
                },
            ),
        };

        self.meshlet_per_frame_resources = Some(PerFrameResources {
            visibility_buffer,
            dummy_render_target,
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

    pub fn resize(&mut self, device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) {
        self.visibility_buffer = {
            let size = wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            };

            device.create_texture(&wgpu::TextureDescriptor {
                label: Some("visibility_buffer"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R64Uint,
                usage: wgpu::TextureUsages::STORAGE_ATOMIC | wgpu::TextureUsages::STORAGE_BINDING,
                view_formats: &[],
            })
        };

        self.material_depth =
            Texture::create_depth_texture(device, config, "material_depth_texture", true);

        self.depth_pyramid = DepthPyramid::new(
            device,
            &self.depth_pyramid.depth_pyramid.label,
            config.width,
            config.height,
        )
    }
}

struct Layout<'a> {
    label: Option<&'a str>,
    entries: Vec<wgpu::BindGroupLayoutEntry>,
}

impl<'a> Layout<'a> {
    fn sequential(
        label: Option<&'a str>,
        visibility: wgpu::ShaderStages,
        entries: &[wgpu::BindGroupLayoutEntry],
    ) -> Self {
        let entries = entries
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let mut entry = layout_entry(i as u32, e.ty);
                entry.visibility = visibility;
                entry
            })
            .collect();

        Self { label, entries }
    }

    fn with(&mut self, mut entry: wgpu::BindGroupLayoutEntry) -> &mut Self {
        entry.binding = self.entries.len() as u32;
        self.entries.push(entry);
        self
    }

    fn get(&self) -> wgpu::BindGroupLayoutDescriptor<'_> {
        wgpu::BindGroupLayoutDescriptor {
            label: self.label,
            entries: &self.entries,
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
