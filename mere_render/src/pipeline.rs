use mere_asset::{Material, Texture, World};

pub struct Pipelines {
    visibility_buffer_clear_pipeline: wgpu::ComputePipeline,
    instance_cull_first_pipeline: wgpu::ComputePipeline,
    instance_cull_second_pipeline: wgpu::ComputePipeline,
    cluster_cull_first_pipeline: wgpu::ComputePipeline,
    cluster_cull_second_pipeline: wgpu::ComputePipeline,
    visibility_buffer_raster_pipeline: wgpu::RenderPipeline,
    fill_counts_pipeline: wgpu::ComputePipeline,
    resolve_material_depth_pipeline: wgpu::RenderPipeline,
    material_pipeline: wgpu::RenderPipeline,
}

impl Pipelines {
    pub fn new(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration, world: &World) -> Self {
        let visibility_buffer_clear_pipeline = {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("visibility_buffer_clear_pipeline_layout"),
                bind_group_layouts: &[Some(
                    &world.resources().visibility_buffer_clear_bind_group_layout,
                )],
                immediate_size: 8,
            });

            create_compute_pipeline(
                Some("visibility_buffer_clear_pipeline"),
                device,
                Some(&layout),
                wgpu::include_wgsl!("visibility_buffer_clear.wgsl"),
                "visibility_buffer_clear",
            )
        };

        let instance_cull_first_pipeline = {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("instance_cull_first_pipeline_layout"),
                bind_group_layouts: &[
                    Some(&world.resources().instance_cull_first_bind_group_layout),
                    Some(&world.resources().render_view_bind_group_layout),
                ],
                immediate_size: 4,
            });

            create_compute_pipeline(
                Some("instance_cull_first_pipeline"),
                device,
                Some(&layout),
                wgpu::include_wgsl!("cull_instances_first.wgsl"),
                "cull_instances",
            )
        };

        let instance_cull_second_pipeline = {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("instance_cull_second_pipeline_layout"),
                bind_group_layouts: &[
                    Some(&world.resources().instance_cull_second_bind_group_layout),
                    Some(&world.resources().render_view_bind_group_layout),
                ],
                immediate_size: 0,
            });

            create_compute_pipeline(
                Some("instance_cull_second_pipeline"),
                device,
                Some(&layout),
                wgpu::include_wgsl!("cull_instances_second.wgsl"),
                "cull_instances",
            )
        };

        let cluster_cull_first_pipeline = {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("cluster_cull_first_pipeline_layout"),
                bind_group_layouts: &[
                    Some(&world.resources().cluster_cull_first_bind_group_layout),
                    Some(&world.resources().render_view_bind_group_layout),
                ],
                immediate_size: 0,
            });

            create_compute_pipeline(
                Some("cluster_cull_first_pipeline"),
                device,
                Some(&layout),
                wgpu::include_wgsl!("cull_clusters_first.wgsl"),
                "cull_clusters",
            )
        };

        let cluster_cull_second_pipeline = {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("cluster_cull_second_pipeline_layout"),
                bind_group_layouts: &[
                    Some(&world.resources().cluster_cull_second_bind_group_layout),
                    Some(&world.resources().render_view_bind_group_layout),
                ],
                immediate_size: 0,
            });

            create_compute_pipeline(
                Some("cluster_cull_second_pipeline"),
                device,
                Some(&layout),
                wgpu::include_wgsl!("cull_clusters_second.wgsl"),
                "cull_clusters",
            )
        };

        let visibility_buffer_raster_pipeline = {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("visibility_buffer_raster_pipeline_layout"),
                bind_group_layouts: &[
                    Some(&world.resources().render_view_bind_group_layout),
                    Some(&world.resources().visibility_buffer_raster_bind_group_layout),
                ],
                immediate_size: 0,
            });

            create_render_pipeline(
                Some("visibility_buffer_raster_pipeline"),
                device,
                Some(&layout),
                Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::R8Uint,
                    blend: None,
                    write_mask: wgpu::ColorWrites::empty(),
                }),
                None,
                wgpu::include_wgsl!("visibility_buffer_raster.wgsl"),
            )
        };

        let fill_counts_pipeline = {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("fill_counts_pipeline_layout"),
                bind_group_layouts: &[Some(&world.resources().fill_counts_bind_group_layout)],
                immediate_size: 0,
            });

            create_compute_pipeline(
                Some("fill_counts_pipeline"),
                device,
                Some(&layout),
                wgpu::include_wgsl!("fill_counts.wgsl"),
                "fill_counts",
            )
        };

        let resolve_material_depth_pipeline = {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("resolve_material_depth_pipeline"),
                bind_group_layouts: &[Some(
                    &world.resources().resolve_material_depth_bind_group_layout,
                )],
                immediate_size: 0,
            });

            create_render_pipeline(
                Some("resolve_material_depth_pipeline"),
                device,
                Some(&layout),
                None,
                Some(wgpu::DepthStencilState {
                    format: Texture::MATERIAL_DEPTH_FORMAT,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::Always),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                wgpu::include_wgsl!("resolve_depth.wgsl"),
            )
        };

        let material_pipeline = {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("material_pipeline_layout"),
                bind_group_layouts: &[
                    Some(&world.resources().render_view_bind_group_layout),
                    Some(&world.resources().meshlet_read_attributes_bind_group_layout),
                    Some(Material::material_bind_group_layout(device)),
                    Some(&world.resources().debug_bind_group_layout),
                ],
                immediate_size: 0,
            });

            create_render_pipeline(
                Some("material_pipeline"),
                device,
                Some(&layout),
                Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::all(),
                }),
                Some(wgpu::DepthStencilState {
                    format: Texture::MATERIAL_DEPTH_FORMAT,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::Equal),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                wgpu::include_wgsl!("material.wgsl"),
            )
        };

        Self {
            visibility_buffer_clear_pipeline,
            instance_cull_first_pipeline,
            instance_cull_second_pipeline,
            cluster_cull_first_pipeline,
            cluster_cull_second_pipeline,
            visibility_buffer_raster_pipeline,
            fill_counts_pipeline,
            resolve_material_depth_pipeline,
            material_pipeline,
        }
    }

    pub fn get(
        &self,
    ) -> (
        &wgpu::ComputePipeline,
        &wgpu::ComputePipeline,
        &wgpu::ComputePipeline,
        &wgpu::ComputePipeline,
        &wgpu::ComputePipeline,
        &wgpu::RenderPipeline,
        &wgpu::ComputePipeline,
        &wgpu::RenderPipeline,
        &wgpu::RenderPipeline,
    ) {
        (
            &self.visibility_buffer_clear_pipeline,
            &self.instance_cull_first_pipeline,
            &self.instance_cull_second_pipeline,
            &self.cluster_cull_first_pipeline,
            &self.cluster_cull_second_pipeline,
            &self.visibility_buffer_raster_pipeline,
            &self.fill_counts_pipeline,
            &self.resolve_material_depth_pipeline,
            &self.material_pipeline,
        )
    }
}

fn create_render_pipeline(
    label: Option<&str>,
    device: &wgpu::Device,
    layout: Option<&wgpu::PipelineLayout>,
    color_target: Option<wgpu::ColorTargetState>,
    depth_target: Option<wgpu::DepthStencilState>,
    shader: wgpu::ShaderModuleDescriptor,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(shader);

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label,
        layout,
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[color_target],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: depth_target,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

fn create_compute_pipeline(
    label: Option<&str>,
    device: &wgpu::Device,
    layout: Option<&wgpu::PipelineLayout>,
    shader: wgpu::ShaderModuleDescriptor,
    entry_point: &str,
) -> wgpu::ComputePipeline {
    let shader = device.create_shader_module(shader);

    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label,
        layout,
        module: &shader,
        entry_point: Some(entry_point),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    })
}
