use crate::{
    asset::Asset, camera::Camera, instance_storage::InstanceStorage,
    meshlet_storage::MeshletStorage, texture::Texture,
};
use depth_pyramid::DepthPyramid;
use mere_mesh::Meshlet;
use render_resources::*;
use wgpu::util::DeviceExt;

mod depth_pyramid;
mod render_resources;

pub use render_resources::PerFrameResources;

#[derive(Clone, Debug)]
pub struct ResourceStorage {
    pub cluster_info: wgpu::Buffer,
    pub visible_cluster_info: wgpu::Buffer,
    pub main_render_view: wgpu::Buffer,
    pub culling_render_view: wgpu::Buffer,
    render_view: RenderView,

    second_pass_instance_candidates: Option<wgpu::Buffer>,
    second_pass_cluster_candidates: wgpu::Buffer,

    pub visibility_buffer: wgpu::TextureView,
    pub dummy_render_target: wgpu::TextureView,
    pub material_depth: Texture,
    dummy_target: wgpu::TextureView,
    pub current_depth_pyramid: DepthPyramid,
    pub previous_depth_pyramid: DepthPyramid,
    pub meshlet_per_frame_resources: Option<PerFrameResources>,

    pub rightmost_slot: u32,
    pub visibility_buffer_clear_bind_group_layout: wgpu::BindGroupLayout,
    pub instance_cull_first_bind_group_layout: wgpu::BindGroupLayout,
    pub instance_cull_second_bind_group_layout: wgpu::BindGroupLayout,
    pub cluster_cull_first_bind_group_layout: wgpu::BindGroupLayout,
    pub cluster_cull_second_bind_group_layout: wgpu::BindGroupLayout,
    pub visibility_buffer_raster_bind_group_layout: wgpu::BindGroupLayout,
    pub fill_counts_bind_group_layout: wgpu::BindGroupLayout,
    pub meshlet_read_attributes_bind_group_layout: wgpu::BindGroupLayout,
    pub render_view_bind_group_layout: wgpu::BindGroupLayout,
    pub resolve_material_depth_bind_group_layout: wgpu::BindGroupLayout,

    pub debug_bind_group_layout: wgpu::BindGroupLayout,
}

impl ResourceStorage {
    pub fn new(
        cluster_slots: u32,
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        main_camera: &Camera,
    ) -> Self {
        let size = wgpu::Extent3d {
            width: config.width,
            height: config.height,
            depth_or_array_layers: 1,
        };

        let visibility_buffer = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("visibility_buffer"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R64Uint,
                usage: wgpu::TextureUsages::STORAGE_ATOMIC | wgpu::TextureUsages::STORAGE_BINDING,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default());

        let dummy_target = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("depth_mips_dummy_texture"),
                size: wgpu::Extent3d::default(),
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R32Float,
                usage: wgpu::TextureUsages::STORAGE_BINDING,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor {
                label: Some("depth_mips_dummy_texture_view"),
                format: Some(wgpu::TextureFormat::R32Float),
                dimension: Some(wgpu::TextureViewDimension::D2),
                usage: None,
                aspect: wgpu::TextureAspect::All,
                base_mip_level: 0,
                mip_level_count: Some(1),
                base_array_layer: 0,
                array_layer_count: Some(1),
            });

        let current_depth_pyramid = DepthPyramid::new(
            device,
            "current_depth_pyramid",
            &visibility_buffer,
            &dummy_target,
        );

        let previous_depth_pyramid = DepthPyramid::new(
            device,
            "previous_depth_pyramid",
            &visibility_buffer,
            &dummy_target,
        );

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
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            culling_render_view: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("culling_render_view"),
                size: size_of::<RenderView>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            render_view: RenderView::new(main_camera),
            second_pass_instance_candidates: None,
            second_pass_cluster_candidates: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("second_pass_cluster_candidates"),
                size: 2 * cluster_slots as u64 * size_of::<u32>() as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            material_depth: Texture::create_depth_texture(
                device,
                config,
                "material_depth_texture",
                true,
            ),
            dummy_target,
            current_depth_pyramid,
            previous_depth_pyramid,
            visibility_buffer,
            dummy_render_target: device
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
                .create_view(&wgpu::TextureViewDescriptor::default()),
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
            instance_cull_first_bind_group_layout: device.create_bind_group_layout(
                &Layout::sequential(
                    Some("instance_cull_first_bind_group_layout"),
                    wgpu::ShaderStages::COMPUTE,
                    &mut [
                        storage_buffer(true),
                        storage_buffer(true),
                        storage_buffer(true),
                        storage_buffer(true),
                        entry(wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        }),
                        storage_buffer(false),
                        storage_buffer(false),
                        storage_buffer(false),
                        storage_buffer(false),
                        storage_buffer(false),
                        storage_buffer(false),
                    ],
                )
                .get(),
            ),
            instance_cull_second_bind_group_layout: device.create_bind_group_layout(
                &Layout::sequential(
                    Some("instance_cull_second_bind_group_layout"),
                    wgpu::ShaderStages::COMPUTE,
                    &mut [
                        storage_buffer(true),
                        storage_buffer(true),
                        storage_buffer(true),
                        storage_buffer(true),
                        entry(wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        }),
                        storage_buffer(false),
                        storage_buffer(false),
                        storage_buffer(false),
                        storage_buffer(true),
                        storage_buffer(true),
                    ],
                )
                .get(),
            ),
            cluster_cull_first_bind_group_layout: device.create_bind_group_layout(
                &Layout::sequential(
                    Some("cluster_cull_first_bind_group_layout"),
                    wgpu::ShaderStages::COMPUTE,
                    &mut [
                        storage_buffer(true),
                        storage_buffer(true),
                        storage_buffer(true),
                        storage_buffer(true),
                        entry(wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        }),
                        storage_buffer(false),
                        storage_buffer(false),
                        storage_buffer(false),
                        storage_buffer(false),
                    ],
                )
                .get(),
            ),
            cluster_cull_second_bind_group_layout: device.create_bind_group_layout(
                &Layout::sequential(
                    Some("cluster_cull_second_bind_group_layout"),
                    wgpu::ShaderStages::COMPUTE,
                    &mut [
                        storage_buffer(true),
                        storage_buffer(true),
                        storage_buffer(true),
                        storage_buffer(true),
                        entry(wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        }),
                        storage_buffer(false),
                        storage_buffer(false),
                        storage_buffer(true),
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
            fill_counts_bind_group_layout: device.create_bind_group_layout(
                &Layout::sequential(
                    Some("fill_counts_bind_group_layout"),
                    wgpu::ShaderStages::COMPUTE,
                    &mut [
                        storage_buffer(false),
                        storage_buffer(false),
                        storage_buffer(false),
                        storage_buffer(false),
                    ],
                )
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
                    &mut [entry(wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    })],
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
        meshlets: &MeshletStorage,
        instances: &InstanceStorage,
    ) {
        std::mem::swap(
            &mut self.current_depth_pyramid,
            &mut self.previous_depth_pyramid,
        );

        let first_pass_cluster_count =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("first_pass_cluster_count"),
                contents: bytemuck::bytes_of(&0u32),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let raster_count = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("raster_count"),
            contents: bytemuck::bytes_of(&0u32),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let needed_buffer_size = instances.scene_instance_count as u64 * size_of::<u32>() as u64;
        let second_pass_instance_candidates = match &mut self.second_pass_instance_candidates {
            Some(buffer) if buffer.size() >= needed_buffer_size => buffer.clone(),
            candidates => {
                let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("second_pass_instance_candidates"),
                    size: needed_buffer_size,
                    usage: wgpu::BufferUsages::STORAGE,
                    mapped_at_creation: false,
                });

                *candidates = Some(buffer.clone());
                buffer
            }
        };

        let second_pass_instance_count =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("second_pass_instance_count"),
                contents: bytemuck::bytes_of(&0u32),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let second_pass_cluster_count =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("second_pass_cluster_count"),
                contents: bytemuck::bytes_of(&0u32),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let instance_second_indirect_args =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("second_instance_indirect_args"),
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

        let cluster_indirect_args = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cluster_indirect_args"),
            contents: wgpu::util::DispatchIndirectArgs { x: 0, y: 1, z: 1 }.as_bytes(),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::INDIRECT,
        });

        let bind_groups = MeshletBindGroups {
            visibility_buffer_clear_bind_group: device.create_bind_group(
                &wgpu::BindGroupDescriptor {
                    label: Some("visibility_buffer_clear_bind_group"),
                    layout: &self.visibility_buffer_clear_bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&self.visibility_buffer),
                    }],
                },
            ),
            instance_cull_first_bind_group: device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("instance_cull_first_bind_group"),
                layout: &self.instance_cull_first_bind_group_layout,
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
                    self.previous_depth_pyramid
                        .depth_pyramid
                        .bind_group_entry_view(4),
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: self.cluster_info.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: first_pass_cluster_count.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 7,
                        resource: cluster_indirect_args.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 8,
                        resource: second_pass_instance_candidates.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 9,
                        resource: second_pass_instance_count.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 10,
                        resource: instance_second_indirect_args.as_entire_binding(),
                    },
                ],
            }),
            instance_cull_second_bind_group: device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("instance_cull_second_bind_group"),
                layout: &self.instance_cull_second_bind_group_layout,
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
                    self.current_depth_pyramid
                        .depth_pyramid
                        .bind_group_entry_view(4),
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: self.second_pass_cluster_candidates.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: second_pass_cluster_count.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 7,
                        resource: cluster_indirect_args.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 8,
                        resource: second_pass_instance_candidates.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 9,
                        resource: second_pass_instance_count.as_entire_binding(),
                    },
                ],
            }),
            cluster_cull_first_bind_group: device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("cluster_cull_first_bind_group"),
                layout: &self.cluster_cull_first_bind_group_layout,
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
                        resource: first_pass_cluster_count.as_entire_binding(),
                    },
                    self.previous_depth_pyramid
                        .depth_pyramid
                        .bind_group_entry_view(4),
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: self.visible_cluster_info.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: indirect_draw_args.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 7,
                        resource: self.second_pass_cluster_candidates.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 8,
                        resource: second_pass_cluster_count.as_entire_binding(),
                    },
                ],
            }),
            cluster_cull_second_bind_group: device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("cluster_cull_second_bind_group"),
                layout: &self.cluster_cull_second_bind_group_layout,
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
                        resource: self.second_pass_cluster_candidates.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: second_pass_cluster_count.as_entire_binding(),
                    },
                    self.current_depth_pyramid
                        .depth_pyramid
                        .bind_group_entry_view(4),
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: self.visible_cluster_info.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: indirect_draw_args.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 7,
                        resource: raster_count.as_entire_binding(),
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
                            resource: raster_count.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 7,
                            resource: wgpu::BindingResource::TextureView(&self.visibility_buffer),
                        },
                    ],
                },
            ),
            fill_counts_bind_group: device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("fill_counts_bind_group"),
                layout: &self.fill_counts_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: cluster_indirect_args.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: indirect_draw_args.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: raster_count.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: second_pass_cluster_count.as_entire_binding(),
                    },
                ],
            }),
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
                            resource: wgpu::BindingResource::TextureView(&self.visibility_buffer),
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
            culling_render_view_bind_group: device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("culling_render_view_bind_group"),
                layout: &self.render_view_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.culling_render_view.as_entire_binding(),
                }],
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
                            resource: wgpu::BindingResource::TextureView(&self.visibility_buffer),
                        },
                    ],
                },
            ),
        };

        self.meshlet_per_frame_resources = Some(PerFrameResources {
            instance_second_indirect_args,
            cluster_indirect_args,
            indirect_draw_args,
            bind_groups,
        });
    }

    pub fn update_render_view(&mut self, queue: &wgpu::Queue, camera: &Camera, update_view: bool) {
        self.render_view.update(camera);
        let binding = [self.render_view];
        let data = bytemuck::cast_slice(&binding);

        queue.write_buffer(&self.main_render_view, 0, data);

        if update_view {
            queue.write_buffer(&self.culling_render_view, 0, data);
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) {
        let size = wgpu::Extent3d {
            width: config.width,
            height: config.height,
            depth_or_array_layers: 1,
        };

        self.visibility_buffer = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("visibility_buffer"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R64Uint,
                usage: wgpu::TextureUsages::STORAGE_ATOMIC | wgpu::TextureUsages::STORAGE_BINDING,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default());

        self.dummy_render_target = device
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

        self.material_depth =
            Texture::create_depth_texture(device, config, "material_depth_texture", true);

        self.current_depth_pyramid = DepthPyramid::new(
            device,
            &self.current_depth_pyramid.depth_pyramid.label,
            &self.visibility_buffer,
            &self.dummy_target,
        );

        self.previous_depth_pyramid = DepthPyramid::new(
            device,
            &self.previous_depth_pyramid.depth_pyramid.label,
            &self.visibility_buffer,
            &self.dummy_target,
        );
    }

    pub fn report_memory_host(&self) -> usize {
        size_of::<Self>()
    }

    pub fn report_memory_device(&self) -> usize {
        let mut total = 0;

        total += self.cluster_info.size() as usize;
        total += self.visible_cluster_info.size() as usize;
        total += self.main_render_view.size() as usize;
        total += self.culling_render_view.size() as usize;
        total += if let Some(buffer) = &self.second_pass_instance_candidates {
            buffer.size() as usize
        } else {
            0
        };
        total += (self.visibility_buffer.texture().width()
            * self.visibility_buffer.texture().height()) as usize
            * size_of::<u64>();
        total += (self.dummy_render_target.texture().width()
            * self.dummy_render_target.texture().height()) as usize
            * size_of::<u8>();
        total += self.material_depth.size_in_bytes();
        total += self.current_depth_pyramid.depth_pyramid.size_in_bytes();
        total += self.previous_depth_pyramid.depth_pyramid.size_in_bytes();
        total += if let Some(per_frame) = self.meshlet_per_frame_resources.as_ref() {
            (per_frame.instance_second_indirect_args.size()
                + per_frame.cluster_indirect_args.size()
                + per_frame.indirect_draw_args.size()) as usize
        } else {
            0
        };
        total
    }
}
