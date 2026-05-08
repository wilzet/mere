use mere_asset::{Material, Texture, World};

pub struct Pipelines {
    instance_cull_pipeline: wgpu::ComputePipeline,
    cluster_cull_pipeline: wgpu::ComputePipeline,
    raster_pipeline: wgpu::RenderPipeline,
    downsample_depth_pipeline: wgpu::ComputePipeline,
}

impl Pipelines {
    pub fn new(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration, world: &World) -> Self {
        let instance_cull_pipeline = {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("instance_cull_pipeline_layout"),
                bind_group_layouts: &[
                    Some(&world.resources().instance_cull_bind_group_layout),
                    Some(&world.resources().render_view_bind_group_layout),
                ],
                immediate_size: 0,
            });

            create_compute_pipeline(
                Some("instance_cull_pipeline"),
                &device,
                Some(&layout),
                wgpu::include_wgsl!("cull_instances.wgsl"),
                "cull_instances",
            )
        };

        let cluster_cull_pipeline = {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("cluster_cull_pipeline_layout"),
                bind_group_layouts: &[
                    Some(&world.resources().cluster_cull_bind_group_layout),
                    Some(&world.resources().render_view_bind_group_layout),
                ],
                immediate_size: 0,
            });

            create_compute_pipeline(
                Some("cluster_cull_pipeline"),
                &device,
                Some(&layout),
                wgpu::include_wgsl!("cull_clusters.wgsl"),
                "cull_clusters",
            )
        };

        let raster_pipeline = {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("raster_pipeline_layout"),
                bind_group_layouts: &[
                    Some(&world.resources().render_view_bind_group_layout),
                    Some(&world.light_bind_group_layout),
                    Some(Material::material_bind_group_layout(&device)),
                    Some(&world.resources().meshlet_mesh_material_bind_group_layout),
                ],
                immediate_size: 0,
            });

            create_render_pipeline(
                Some("raster_pipeline"),
                &device,
                Some(&layout),
                config.format,
                Some(Texture::DEPTH_FORMAT),
                &[],
                Some(wgpu::BlendState::REPLACE),
                wgpu::include_wgsl!("meshlet_debug.wgsl"),
            )
        };

        let downsample_depth_pipeline = {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("depth_downsample_pipeline_layout"),
                bind_group_layouts: &[Some(&world.resources().depth_downsample_bind_group_layout)],
                immediate_size: 0,
            });

            create_compute_pipeline(
                Some("depth_downsample_pipeline"),
                device,
                Some(&layout),
                wgpu::include_wgsl!("depth_downsample.wgsl"),
                "main",
            )
        };

        Self {
            instance_cull_pipeline,
            cluster_cull_pipeline,
            raster_pipeline,
            downsample_depth_pipeline,
        }
    }

    pub fn get(
        &self,
    ) -> (
        &wgpu::ComputePipeline,
        &wgpu::ComputePipeline,
        &wgpu::RenderPipeline,
        &wgpu::ComputePipeline,
    ) {
        (
            &self.instance_cull_pipeline,
            &self.cluster_cull_pipeline,
            &self.raster_pipeline,
            &self.downsample_depth_pipeline,
        )
    }
}

fn create_render_pipeline(
    label: Option<&str>,
    device: &wgpu::Device,
    layout: Option<&wgpu::PipelineLayout>,
    color_format: wgpu::TextureFormat,
    depth_format: Option<wgpu::TextureFormat>,
    vertex_layouts: &[wgpu::VertexBufferLayout],
    blend: Option<wgpu::BlendState>,
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
            buffers: vertex_layouts,
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: color_format,
                blend,
                write_mask: wgpu::ColorWrites::ALL,
            })],
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
        depth_stencil: depth_format.map(|format| wgpu::DepthStencilState {
            format,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
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
