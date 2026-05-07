use crate::{
    CLUSTER_SLOTS,
    lights::LightUniform,
    pipeline::{create_compute_pipeline, create_render_pipeline},
};
use mere_asset::{InstanceStorage, Material, ResourceStorage, Texture, World};
use mere_log::Profiler;
use mere_math::{Quat, Vec3};
use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::window::Window;

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    is_surface_configured: bool,
    instance_cull_pipeline: wgpu::ComputePipeline,
    cluster_cull_pipeline: wgpu::ComputePipeline,
    raster_pipeline: wgpu::RenderPipeline,
    depth_texture: Texture,
    bg_color: wgpu::Color,
    light_uniform: LightUniform,
    light_buffer: wgpu::Buffer,
    light_bind_group: wgpu::BindGroup,
}

impl Renderer {
    pub async fn new(window: Arc<Window>) -> anyhow::Result<(Self, World)> {
        let size = window.inner_size();
        mere_log::info!("Initializing WGPU ({}x{})", size.width, size.height);

        let gpu_instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            flags: Default::default(),
            memory_budget_thresholds: Default::default(),
            backend_options: Default::default(),
            display: None,
        });

        let surface = gpu_instance.create_surface(window.clone()).unwrap();

        let adapter = gpu_instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await?;

        let info = adapter.get_info();
        mere_log::success!("Selected adapter: {} ({:?})", info.name, info.backend);

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Rendering device"),
                required_features: wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES
                    | wgpu::Features::POLYGON_MODE_LINE
                    | wgpu::Features::TIMESTAMP_QUERY,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                required_limits: wgpu::Limits {
                    max_compute_workgroup_size_x: 1024,
                    max_compute_workgroup_size_y: 1024,
                    max_compute_workgroup_size_z: 64,
                    max_compute_invocations_per_workgroup: 1024,
                    ..Default::default()
                },
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
            })
            .await?;

        let world = World::new(&device, &queue, CLUSTER_SLOTS);

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or_else(|| {
                let format = surface_caps.formats[0];
                mere_log::warn!("sRGB format not found, falling back to {format:?}");
                format
            });

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::AutoNoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };

        let light_uniform = LightUniform {
            position: [1.5, 2.0, 1.5],
            _padding: 0,
            color: [1.0, 1.0, 1.0],
            _padding2: 0,
        };

        let light_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Light VB"),
            contents: bytemuck::cast_slice(&[light_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let light_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let light_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &light_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: light_buffer.as_entire_binding(),
            }],
        });

        let depth_texture = Texture::create_depth_texture(&device, &config, "depth_texture");

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
                    Some(&light_bind_group_layout),
                    Some(Material::material_bind_group_layout(&device)),
                    Some(&world.resources().meshlet_mesh_material_bind_group_layout),
                ],
                immediate_size: 0,
            });

            create_render_pipeline(
                Some("raster_pipeline"),
                &device,
                &layout,
                config.format,
                Some(Texture::DEPTH_FORMAT),
                &[],
                Some(wgpu::BlendState::REPLACE),
                wgpu::include_wgsl!("meshlet_debug.wgsl"),
            )
        };

        Ok((
            Self {
                surface,
                device,
                queue,
                config,
                is_surface_configured: false,
                instance_cull_pipeline,
                cluster_cull_pipeline,
                raster_pipeline,
                depth_texture,
                bg_color: wgpu::Color {
                    r: 0.1,
                    g: 0.2,
                    b: 0.3,
                    a: 1.0,
                },
                light_uniform,
                light_buffer,
                light_bind_group,
            },
            world,
        ))
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);

        self.depth_texture =
            Texture::create_depth_texture(&self.device, &self.config, "depth_texture");

        self.is_surface_configured = true;
    }

    pub fn update_light(&mut self, delta_time: f32) {
        let old_position = Vec3::from(self.light_uniform.position);
        self.light_uniform.position =
            (Quat::from_axis_angle(Vec3::Y, delta_time * 36.0f32.to_radians()) * old_position)
                .into();

        self.queue.write_buffer(
            &self.light_buffer,
            0,
            bytemuck::cast_slice(&[self.light_uniform]),
        );
    }

    pub fn set_bg_color(&mut self, color: wgpu::Color) {
        self.bg_color = color;
    }

    pub fn get_device_queue(&self) -> (&wgpu::Device, &wgpu::Queue) {
        (&self.device, &self.queue)
    }

    pub fn get_config(&self) -> &wgpu::SurfaceConfiguration {
        &self.config
    }

    pub fn create_encoder(
        &mut self,
    ) -> anyhow::Result<
        Option<(
            wgpu::SurfaceTexture,
            wgpu::TextureView,
            wgpu::CommandEncoder,
        )>,
    > {
        if !self.is_surface_configured {
            return Ok(None);
        }

        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(surface_texture) => surface_texture,
            wgpu::CurrentSurfaceTexture::Suboptimal(surface_texture) => {
                self.surface.configure(&self.device, &self.config);
                surface_texture
            }
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => return Ok(None),
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return Ok(None);
            }
            wgpu::CurrentSurfaceTexture::Lost => anyhow::bail!("device lost"),
        };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        Ok(Some((output, view, encoder)))
    }

    pub fn render(
        &self,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        instances: &InstanceStorage,
        resources: &ResourceStorage,
        material: &Material,
        profiler: &mut Profiler,
    ) {
        let Some(per_frame_resources) = resources.meshlet_per_frame_resources.as_ref() else {
            return;
        };

        {
            let timestamp_writes = profiler.begin("instance_cull_pass");

            let mut instance_cull_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("instance_cull_pass"),
                timestamp_writes,
            });
            instance_cull_pass.set_pipeline(&self.instance_cull_pipeline);
            instance_cull_pass.set_bind_group(
                0,
                &per_frame_resources.bind_groups.instance_cull_bind_group,
                &[],
            );
            instance_cull_pass.set_bind_group(
                1,
                &per_frame_resources.bind_groups.render_view_bind_group,
                &[],
            );
            instance_cull_pass.dispatch_workgroups(instances.scene_instance_count, 1, 1);

            profiler.end();
        }

        {
            let timestamp_writes = profiler.begin("cluster_cull_pass");

            let mut cluster_cull_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("cluster_cull_pass"),
                timestamp_writes,
            });
            cluster_cull_pass.set_pipeline(&self.cluster_cull_pipeline);
            cluster_cull_pass.set_bind_group(
                0,
                &per_frame_resources.bind_groups.cluster_cull_bind_group,
                &[],
            );
            cluster_cull_pass.set_bind_group(
                1,
                &per_frame_resources.bind_groups.render_view_bind_group,
                &[],
            );

            cluster_cull_pass
                .dispatch_workgroups_indirect(&per_frame_resources.indirect_cluster_args, 0);

            profiler.end();
        }

        {
            let timestamp_writes = profiler.begin("raster_pass");

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("raster_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.bg_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes,
                multiview_mask: None,
            });

            render_pass.set_pipeline(&self.raster_pipeline);
            render_pass.set_bind_group(
                0,
                &per_frame_resources.bind_groups.main_render_view_bind_group,
                &[],
            );
            render_pass.set_bind_group(1, &self.light_bind_group, &[]);
            render_pass.set_bind_group(2, &material.bind_group, &[]);
            render_pass.set_bind_group(
                3,
                &per_frame_resources
                    .bind_groups
                    .meshlet_mesh_material_bind_group,
                &[],
            );

            render_pass.draw_indirect(&per_frame_resources.indirect_draw_args, 0);

            profiler.end();
        }
    }
}
