use crate::{
    camera::{CameraController, CameraUniform},
    instance::InstanceRaw,
    lights::LightUniform,
    model::{DrawItem, DrawLight, DrawModel},
    renderer::create_render_pipeline,
};
use mere_asset::{Camera, Material, Scene, Texture};
use mere_math::{Quat, Vec3};
use mere_mesh::Vertex;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use wgpu::util::DeviceExt;
use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};

mod camera;
mod instance;
mod lights;
mod model;
mod renderer;

pub struct State {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    is_surface_configured: bool,
    opaque_render_pipeline: wgpu::RenderPipeline,
    alpha_render_pipeline: wgpu::RenderPipeline,
    light_pipeline: wgpu::RenderPipeline,
    window: Arc<Window>,
    lock_cursor: bool,
    camera: Camera,
    camera_uniform: CameraUniform,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    camera_controller: CameraController,
    stored_cursor_pos: (f64, f64),
    depth_texture: Texture,
    bg_color: wgpu::Color,
    light_uniform: LightUniform,
    light_buffer: wgpu::Buffer,
    light_bind_group: wgpu::BindGroup,
    instance_buffer: wgpu::Buffer,
    scene: Scene,
}

impl State {
    pub async fn new(window: Arc<Window>) -> anyhow::Result<Self> {
        let size = window.inner_size();
        mere_log::info!("Initializing WGPU ({}x{})", size.width, size.height);

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            flags: Default::default(),
            memory_budget_thresholds: Default::default(),
            backend_options: Default::default(),
            display: None,
        });

        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = instance
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
                required_features: wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                required_limits: wgpu::Limits::default(),
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
            present_mode: wgpu::PresentMode::AutoNoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };

        let camera_controller = CameraController::new(5.0, 0.002);

        let mut camera = Camera::new(
            45.0f32.to_radians(),
            config.width as f32 / config.height as f32,
            0.1,
            100.0,
            Vec3::new(0.0, 1.0, 2.0),
        );
        camera.look_at(Vec3::ZERO);

        let mut camera_uniform = CameraUniform::new();
        camera_uniform.update_view_proj(&camera);

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("camera_bind_group_layout"),
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

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera_bind_group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        let mut scene = Scene::new(&device, &queue);
        scene.load_gltf("sponza/main_sponza", &device, &queue)?;
        scene.load_gltf("sponza/pkg_a_curtains", &device, &queue)?;

        let instances = scene
            .objects()
            .map(|obj| InstanceRaw::from_transform(obj.transform))
            .collect::<Vec<_>>();

        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Instance Buffer"),
            contents: bytemuck::cast_slice(&instances),
            usage: wgpu::BufferUsages::VERTEX,
        });

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

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[
                    Some(&camera_bind_group_layout),
                    Some(&light_bind_group_layout),
                    Some(Material::material_bind_group_layout(&device)),
                ],
                immediate_size: 0,
            });
        let forward_main_shader = wgpu::include_wgsl!("shader.wgsl");

        let opaque_render_pipeline = create_render_pipeline(
            &device,
            &render_pipeline_layout,
            config.format,
            Some(Texture::DEPTH_FORMAT),
            &[Vertex::desc(), InstanceRaw::desc()],
            Some(wgpu::BlendState::REPLACE),
            forward_main_shader.clone(),
        );

        let alpha_render_pipeline = create_render_pipeline(
            &device,
            &render_pipeline_layout,
            config.format,
            Some(Texture::DEPTH_FORMAT),
            &[Vertex::desc(), InstanceRaw::desc()],
            Some(wgpu::BlendState::ALPHA_BLENDING),
            forward_main_shader,
        );

        let light_pipeline = {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Light Pipeline Layout"),
                bind_group_layouts: &[
                    Some(&camera_bind_group_layout),
                    Some(&light_bind_group_layout),
                ],
                immediate_size: 0,
            });

            let shader = wgpu::include_wgsl!("light.wgsl");

            create_render_pipeline(
                &device,
                &layout,
                config.format,
                Some(Texture::DEPTH_FORMAT),
                &[Vertex::desc()],
                Some(wgpu::BlendState::REPLACE),
                shader,
            )
        };

        mere_log::success!("State initialization complete.");

        Ok(Self {
            surface,
            device,
            queue,
            config,
            is_surface_configured: false,
            opaque_render_pipeline,
            alpha_render_pipeline,
            light_pipeline,
            window,
            lock_cursor: false,
            camera,
            camera_uniform,
            camera_buffer,
            camera_bind_group,
            camera_controller,
            stored_cursor_pos: (0.0, 0.0),
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
            instance_buffer,
            scene,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);

            self.depth_texture =
                Texture::create_depth_texture(&self.device, &self.config, "depth_texture");

            self.is_surface_configured = true;
        }
    }

    pub fn handle_key(&mut self, event_loop: &ActiveEventLoop, code: KeyCode, is_pressed: bool) {
        match (code, is_pressed) {
            (KeyCode::Escape, true) => event_loop.exit(),
            _ => {
                self.camera_controller.handle_key(code, is_pressed);
            }
        }
    }

    pub fn handle_mouse_moved(&mut self, x: f64, y: f64) {
        let (width, height): (f64, f64) = self.window.inner_size().into();

        if !self.lock_cursor {
            let r = (x / width).clamp(0.0, 1.0);
            let g = (y / height).clamp(0.0, 1.0);
            self.bg_color = wgpu::Color {
                r,
                g,
                b: 0.3,
                a: 1.0,
            };

            self.stored_cursor_pos = (x, y);
            return;
        }

        let center_x = width * 0.5;
        let center_y = height * 0.5;

        let dx = x - center_x;
        let dy = y - center_y;
        if dx.abs() < 0.1 && dy.abs() < 0.1 {
            return;
        }

        self.camera_controller.handle_mouse_move(dx, dy);

        let _ = self
            .window
            .set_cursor_position(winit::dpi::PhysicalPosition::new(center_x, center_y));
    }

    pub fn handle_mouse_input(&mut self, button: MouseButton, is_pressed: bool) {
        if let MouseButton::Left = button {
            self.lock_cursor = is_pressed;
            self.window.set_cursor_visible(!is_pressed);

            if is_pressed {
                let (width, height): (f64, f64) = self.window.inner_size().into();
                let _ = self
                    .window
                    .set_cursor_position(winit::dpi::PhysicalPosition::new(
                        width * 0.5,
                        height * 0.5,
                    ));
            } else {
                let _ = self
                    .window
                    .set_cursor_position(winit::dpi::PhysicalPosition::new(
                        self.stored_cursor_pos.0,
                        self.stored_cursor_pos.1,
                    ));
            }
        }
    }

    pub fn handle_mouse_scroll(&mut self, delta: MouseScrollDelta) {
        self.camera_controller.handle_mouse_scroll(&delta)
    }

    pub fn update(&mut self, delta_time: Duration) {
        let dt = delta_time.as_secs_f32();

        self.scene.process_asset_event();
        self.camera_controller.update_camera(&mut self.camera, dt);
        self.camera_uniform.update_view_proj(&self.camera);
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[self.camera_uniform]),
        );

        let old_position = Vec3::from(self.light_uniform.position);
        self.light_uniform.position =
            (Quat::from_axis_angle(Vec3::Y, dt * 36.0f32.to_radians()) * old_position).into();
        self.queue.write_buffer(
            &self.light_buffer,
            0,
            bytemuck::cast_slice(&[self.light_uniform]),
        );
    }

    pub fn render(&mut self) -> anyhow::Result<()> {
        if !self.is_surface_configured {
            return Ok(());
        }

        let default_lock = self.scene.get_material(Material::DEFAULT_MATERIAL_ID);
        let default_material = default_lock.read();
        let default_bg = default_material.bind_group.as_ref().unwrap();

        let mut opaque_draw_items = Vec::new();
        let mut alpha_draw_items = Vec::new();
        for (i, object) in self.scene.objects().enumerate() {
            if let Some(model) = self.scene.get_model(object.handle()) {
                for mesh in model.read().meshes() {
                    let lock = self.scene.get_material(mesh.material);
                    let material = lock.read();
                    let bind_group = match &material.bind_group {
                        Some(bg) => bg,
                        None => default_bg,
                    };

                    let item = DrawItem {
                        instance_index: i as u32,
                        mesh: mesh.clone(),
                        material: bind_group.clone(),
                    };

                    if material.alpha_blended {
                        alpha_draw_items.push(item);
                    } else {
                        opaque_draw_items.push(item);
                    }
                }
            }
        }

        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(surface_texture) => surface_texture,
            wgpu::CurrentSurfaceTexture::Suboptimal(surface_texture) => {
                self.surface.configure(&self.device, &self.config);
                surface_texture
            }
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => return Ok(()),
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Lost => anyhow::bail!("device lost"),
        };
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
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
                timestamp_writes: None,
                multiview_mask: None,
            });

            render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));

            render_pass.set_pipeline(&self.light_pipeline);
            if let Some(mesh) = opaque_draw_items.get(26) {
                render_pass.draw_light_mesh(
                    mesh.clone(),
                    &self.camera_bind_group,
                    &self.light_bind_group,
                );
            }

            render_pass.set_pipeline(&self.opaque_render_pipeline);
            render_pass.draw_meshes(
                opaque_draw_items,
                &self.camera_bind_group,
                &self.light_bind_group,
            );

            render_pass.set_pipeline(&self.alpha_render_pipeline);
            render_pass.draw_meshes(
                alpha_draw_items,
                &self.camera_bind_group,
                &self.light_bind_group,
            );
        }

        self.queue.submit(Some(encoder.finish()));
        output.present();

        Ok(())
    }
}

pub struct App {
    state: Option<State>,
    last_frame_time: Instant,
}

impl App {
    pub fn new() -> Self {
        Self {
            state: None,
            last_frame_time: Instant::now(),
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_attributes = Window::default_attributes().with_title("MeRe");
        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());

        self.state = match pollster::block_on(State::new(window)) {
            Ok(state) => Some(state),
            Err(err) => {
                mere_log::error!("{err}");
                event_loop.exit();
                None
            }
        };
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let state = match &mut self.state {
            Some(canvas) => canvas,
            None => return,
        };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => state.resize(size.width, size.height),
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = now - self.last_frame_time;
                self.last_frame_time = now;
                state.update(dt);
                if let Err(err) = state.render() {
                    mere_log::error!("{err}");
                    event_loop.exit();
                }
                state.window.request_redraw();
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state: key_state,
                        ..
                    },
                ..
            } => state.handle_key(event_loop, code, key_state.is_pressed()),
            WindowEvent::CursorMoved { position, .. } => {
                state.handle_mouse_moved(position.x, position.y)
            }
            WindowEvent::MouseInput {
                state: key_state,
                button,
                ..
            } => state.handle_mouse_input(button, key_state.is_pressed()),
            WindowEvent::MouseWheel { delta, .. } => state.handle_mouse_scroll(delta),
            _ => (),
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        mere_log::info!("Shutting down...")
    }
}

pub fn run() -> anyhow::Result<()> {
    env_logger::init();

    let event_loop = EventLoop::builder().build()?;
    let mut app = App::new();

    match event_loop.run_app(&mut app) {
        Ok(_) => (),
        Err(err) => mere_log::error!(return err),
    }

    Ok(())
}
