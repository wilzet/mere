use crate::{CLUSTER_SLOTS, Debug, pipeline::Pipelines};
use mere_asset::{MaterialData, PerFrameResources, ResourceStorage, World};
use mere_log::Profiler;
use std::sync::Arc;
use winit::window::Window;

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    is_surface_configured: bool,
    pipelines: Pipelines,
    bg_color: wgpu::Color,
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
                    | wgpu::Features::TEXTURE_ATOMIC
                    | wgpu::Features::TEXTURE_INT64_ATOMIC
                    | wgpu::Features::SHADER_INT64
                    | wgpu::Features::IMMEDIATES
                    | wgpu::Features::TIMESTAMP_QUERY
                    | wgpu::Features::FLOAT32_FILTERABLE,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                required_limits: wgpu::Limits {
                    max_compute_workgroup_size_x: 1024,
                    max_compute_workgroup_size_y: 1024,
                    max_compute_workgroup_size_z: 64,
                    max_compute_invocations_per_workgroup: 1024,
                    max_storage_textures_per_shader_stage: 16,
                    max_storage_buffers_per_shader_stage: 16,
                    max_immediate_size: 8,
                    max_storage_buffer_binding_size: 1024 * 1024 * 1024,
                    max_buffer_size: 1024 * 1024 * 1024,
                    ..Default::default()
                },
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
            })
            .await?;

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
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };

        let world = World::new(&device, &queue, &config, CLUSTER_SLOTS);

        let pipelines = Pipelines::new(&device, &config, &world);

        Ok((
            Self {
                surface,
                device,
                queue,
                config,
                is_surface_configured: false,
                pipelines,
                bg_color: wgpu::Color {
                    r: 0.1,
                    g: 0.2,
                    b: 0.3,
                    a: 1.0,
                },
            },
            world,
        ))
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);

        self.is_surface_configured = true;
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
                label: Some("meshlet_render_encoder"),
            });

        Ok(Some((output, view, encoder)))
    }

    pub fn render(
        &self,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        instance_count: u32,
        resources: &ResourceStorage,
        materials: &[MaterialData],
        profiler: &mut Profiler,
        lock_view: bool,
        debug: &Debug,
    ) {
        let Some(per_frame_resources) = resources.meshlet_per_frame_resources.as_ref() else {
            return;
        };

        let (
            visibility_buffer_clear_pipeline,
            instance_cull_first_pipeline,
            instance_cull_second_pipeline,
            cluster_cull_first_pipeline,
            cluster_cull_second_pipeline,
            visibility_buffer_raster_pipeline,
            fill_counts_pipeline,
            resolve_material_depth_pipeline,
            material_pipeline,
        ) = self.pipelines.get();

        {
            let timestamp_writes = profiler.begin("visibility_buffer_clear_pass");

            let mut visibility_buffer_clear_pass =
                encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("visibility_buffer_clear_pass"),
                    timestamp_writes,
                });
            visibility_buffer_clear_pass.set_pipeline(visibility_buffer_clear_pipeline);
            visibility_buffer_clear_pass.set_bind_group(
                0,
                &per_frame_resources
                    .bind_groups
                    .visibility_buffer_clear_bind_group,
                &[],
            );
            let size = view.texture().size();
            visibility_buffer_clear_pass
                .set_immediates(0, bytemuck::cast_slice(&[size.width, size.height]));

            visibility_buffer_clear_pass.dispatch_workgroups(
                size.width.div_ceil(16),
                size.height.div_ceil(16),
                1,
            );

            profiler.end();
        }

        cull_pass(
            true,
            encoder,
            per_frame_resources,
            &resources.dummy_render_target,
            instance_count,
            instance_cull_first_pipeline,
            cluster_cull_first_pipeline,
            visibility_buffer_raster_pipeline,
            fill_counts_pipeline,
            profiler,
        );

        if !lock_view {
            resources
                .current_depth_pyramid
                .downsample(encoder, profiler);
        }

        cull_pass(
            false,
            encoder,
            per_frame_resources,
            &resources.dummy_render_target,
            instance_count,
            instance_cull_second_pipeline,
            cluster_cull_second_pipeline,
            visibility_buffer_raster_pipeline,
            fill_counts_pipeline,
            profiler,
        );

        if !lock_view {
            resources
                .current_depth_pyramid
                .downsample(encoder, profiler);
        }

        {
            let timestamp_writes = profiler.begin("resolve_material_depth_pass");

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("resolve_material_depth_pass"),
                color_attachments: &[None],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &resources.material_depth.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            render_pass.set_pipeline(resolve_material_depth_pipeline);
            render_pass.set_bind_group(
                0,
                &per_frame_resources
                    .bind_groups
                    .resolve_material_depth_bind_group,
                &[],
            );
            render_pass.draw(0..3, 0..1);

            profiler.end();
        }

        {
            let timestamp_writes = profiler.begin("material_pass");

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("material_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.bg_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &resources.material_depth.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            render_pass.set_pipeline(material_pipeline);
            render_pass.set_bind_group(
                0,
                &per_frame_resources.bind_groups.main_render_view_bind_group,
                &[],
            );
            render_pass.set_bind_group(
                1,
                &per_frame_resources
                    .bind_groups
                    .meshlet_read_attributes_bind_group,
                &[],
            );
            render_pass.set_bind_group(3, Some(&debug.bind_group), &[]);

            for material in materials {
                render_pass.set_bind_group(2, Some(material.bind_group.as_ref()), &[]);
                let x = material.id * 3;
                render_pass.draw(x..(x + 3), 0..1);
            }

            profiler.end();
        }
    }
}

fn cull_pass(
    first: bool,
    encoder: &mut wgpu::CommandEncoder,
    per_frame_resources: &PerFrameResources,
    render_target: &wgpu::TextureView,
    instance_count: u32,
    instance_cull_pipeline: &wgpu::ComputePipeline,
    cluster_cull_pipeline: &wgpu::ComputePipeline,
    raster_pipeline: &wgpu::RenderPipeline,
    fill_counts_pipeline: &wgpu::ComputePipeline,
    profiler: &mut Profiler,
) {
    {
        let name = if first {
            "instance_cull_first_pass"
        } else {
            "instance_cull_second_pass"
        };
        let timestamp_writes = profiler.begin(name);

        let mut instance_cull_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some(name),
            timestamp_writes,
        });
        instance_cull_pass.set_pipeline(instance_cull_pipeline);

        let bind_group = if first {
            &per_frame_resources
                .bind_groups
                .instance_cull_first_bind_group
        } else {
            &per_frame_resources
                .bind_groups
                .instance_cull_second_bind_group
        };
        instance_cull_pass.set_bind_group(0, bind_group, &[]);

        instance_cull_pass.set_bind_group(
            1,
            &per_frame_resources
                .bind_groups
                .culling_render_view_bind_group,
            &[],
        );

        if first {
            instance_cull_pass.set_immediates(0, &instance_count.to_le_bytes());

            const MAX_WORKGROUP_DIM: u32 = 65535;
            let (x, y) = if instance_count <= MAX_WORKGROUP_DIM {
                (instance_count, 1)
            } else {
                (
                    MAX_WORKGROUP_DIM,
                    instance_count.div_ceil(MAX_WORKGROUP_DIM),
                )
            };

            instance_cull_pass.dispatch_workgroups(x, y, 1);
        } else {
            instance_cull_pass.dispatch_workgroups_indirect(
                &per_frame_resources.instance_second_indirect_args,
                0,
            );
        }

        profiler.end();
    }

    {
        let name = if first {
            "cluster_cull_first_pass"
        } else {
            "cluster_cull_second_pass"
        };
        let timestamp_writes = profiler.begin(name);

        let mut cluster_cull_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some(name),
            timestamp_writes,
        });
        cluster_cull_pass.set_pipeline(cluster_cull_pipeline);

        let bind_group = if first {
            &per_frame_resources
                .bind_groups
                .cluster_cull_first_bind_group
        } else {
            &per_frame_resources
                .bind_groups
                .cluster_cull_second_bind_group
        };
        cluster_cull_pass.set_bind_group(0, bind_group, &[]);

        cluster_cull_pass.set_bind_group(
            1,
            &per_frame_resources
                .bind_groups
                .culling_render_view_bind_group,
            &[],
        );

        cluster_cull_pass
            .dispatch_workgroups_indirect(&per_frame_resources.cluster_indirect_args, 0);

        profiler.end();
    }

    {
        let name = if first {
            "visibility_buffer_first_raster_pass"
        } else {
            "visibility_buffer_second_raster_pass"
        };
        let timestamp_writes = profiler.begin(name);

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(name),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: render_target,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Discard,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes,
            multiview_mask: None,
        });

        render_pass.set_pipeline(raster_pipeline);
        render_pass.set_bind_group(
            0,
            &per_frame_resources.bind_groups.main_render_view_bind_group,
            &[],
        );
        render_pass.set_bind_group(
            1,
            &per_frame_resources
                .bind_groups
                .visibility_buffer_raster_bind_group,
            &[],
        );

        render_pass.draw_indirect(&per_frame_resources.indirect_draw_args, 0);

        profiler.end();
    }

    {
        let name = if first {
            "fill_counts_first_pass"
        } else {
            "fill_counts_second_pass"
        };
        let timestamp_writes = profiler.begin(name);

        let mut fill_counts_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some(name),
            timestamp_writes,
        });

        fill_counts_pass.set_pipeline(fill_counts_pipeline);
        fill_counts_pass.set_bind_group(
            0,
            &per_frame_resources.bind_groups.fill_counts_bind_group,
            &[],
        );

        fill_counts_pass.dispatch_workgroups(1, 1, 1);

        profiler.end();
    }
}
