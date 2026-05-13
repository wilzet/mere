use crate::{camera::CameraController, egui_debugger::EguiRenderer, renderer::Renderer};
use mere_asset::World;
use mere_log::Profiler;
use mere_math::{Transform, Vec3};
use std::{sync::Arc, time::Duration};
use wgpu::util::DeviceExt;
use winit::{
    event::*,
    event_loop::ActiveEventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};

mod camera;
mod egui_debugger;
mod pipeline;
mod renderer;

pub const CLUSTER_SLOTS: u32 = 1 << 20;

#[derive(PartialEq, Clone, Copy, Default, Debug)]
pub enum DebugMode {
    #[default]
    CLUSTERS,
    SHADED,
    TRIANGLES,
    INSTANCES,
    MATERIALS,
}

impl DebugMode {
    pub fn name(&self) -> &str {
        match self {
            Self::SHADED => "Shaded",
            Self::CLUSTERS => "Clusters",
            Self::TRIANGLES => "Triangles",
            Self::INSTANCES => "Instances",
            Self::MATERIALS => "Materials",
        }
    }
}

pub struct Debug {
    pub mode: DebugMode,
    pub debug_buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
}

impl Debug {
    fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Self {
        let debug_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("debug_buffer"),
            contents: bytemuck::cast_slice(&[0u32]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let debug_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("debug_bind_group"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: debug_buffer.as_entire_binding(),
            }],
        });

        Self {
            mode: DebugMode::default(),
            debug_buffer,
            bind_group: debug_bind_group,
        }
    }
}

pub struct State {
    mere_renderer: Renderer,
    window: Arc<Window>,
    lock_cursor: bool,
    camera_controller: CameraController,
    stored_cursor_pos: (f64, f64),
    world: World,
    lock_view: bool,
    egui_renderer: EguiRenderer,
    profiler: Profiler,
    debug: Debug,
}

impl State {
    pub async fn new(window: Arc<Window>) -> anyhow::Result<Self> {
        let (mere_renderer, mut world) = Renderer::new(window.clone()).await?;

        let (device, queue) = mere_renderer.get_device_queue();
        let config = mere_renderer.get_config();

        //world.load_gltf("sponza/main_sponza", device, queue)?;
        //world.load_gltf("sponza/pkg_a_curtains", device, queue)?;
        let teapot_handle = world.load_gltf("utah_teapot", device, queue)?[0];
        let teapot = world.get_instance(teapot_handle).unwrap().clone();
        for x in 0..10 {
            for y in 0..10 {
                for z in 0..10 {
                    if x == 0 && y == 0 && z == 0 {
                        continue;
                    }

                    world.add_instance(
                        Transform::new()
                            .with_translation(Vec3::new(x as f32, y as f32, z as f32) * 6.0)
                            .with_rotation(teapot.transform.rotation),
                        teapot.meshlet_mesh,
                        teapot.material,
                    );
                }
            }
        }

        let camera_controller = CameraController::new(5.0, 0.002);

        let egui_renderer = EguiRenderer::new(&device, config.format, None, 1, &window);
        let profiler = Profiler::new(device);

        let debug = Debug::new(device, &world.resources().debug_bind_group_layout);

        mere_log::success!("State initialization complete.");

        Ok(Self {
            mere_renderer,
            window,
            lock_cursor: false,
            camera_controller,
            stored_cursor_pos: (0.0, 0.0),
            world,
            lock_view: false,
            egui_renderer,
            profiler,
            debug,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.mere_renderer.resize(width, height);

            let (device, _) = self.mere_renderer.get_device_queue();
            let config = self.mere_renderer.get_config();
            self.world.resize(device, config);
        }
    }

    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }

    pub fn handle_input(&mut self, event_loop: &ActiveEventLoop, event: &WindowEvent) {
        let ui_input = if !self.lock_cursor {
            self.egui_renderer.handle_input(&self.window, &event)
        } else {
            false
        };

        match *event {
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state: key_state,
                        ..
                    },
                ..
            } => self.handle_key(event_loop, code, key_state.is_pressed()),
            WindowEvent::CursorMoved { position, .. } if !ui_input => {
                self.handle_mouse_moved(position.x, position.y)
            }
            WindowEvent::MouseInput {
                state: key_state,
                button,
                ..
            } if !ui_input => self.handle_mouse_input(button, key_state.is_pressed()),
            WindowEvent::MouseWheel { delta, .. } if !ui_input => self.handle_mouse_scroll(delta),
            _ => (),
        }
    }

    fn handle_key(&mut self, event_loop: &ActiveEventLoop, code: KeyCode, is_pressed: bool) {
        match (code, is_pressed) {
            (KeyCode::Escape, true) => event_loop.exit(),
            _ => {
                self.camera_controller.handle_key(code, is_pressed);
            }
        }
    }

    fn handle_mouse_moved(&mut self, x: f64, y: f64) {
        let (width, height): (f64, f64) = self.window.inner_size().into();

        if !self.lock_cursor {
            let r = (x / width).clamp(0.0, 1.0);
            let g = (y / height).clamp(0.0, 1.0);
            self.mere_renderer.set_bg_color(wgpu::Color {
                r,
                g,
                b: 0.3,
                a: 1.0,
            });

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

    fn handle_mouse_input(&mut self, button: MouseButton, is_pressed: bool) {
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

    fn handle_mouse_scroll(&mut self, delta: MouseScrollDelta) {
        self.camera_controller.handle_mouse_scroll(&delta)
    }

    pub fn update(&mut self, delta_time: Duration) {
        let dt = delta_time.as_secs_f32();

        self.world.process_asset_event();

        let (device, queue) = self.mere_renderer.get_device_queue();
        let config = self.mere_renderer.get_config();
        self.world.prepare_meshlet_resources(
            device,
            queue,
            config,
            !self.lock_view,
            &mut self.profiler,
        );

        self.camera_controller
            .update_camera(self.world.main_camera_mut(), dt);
    }

    pub fn render(&mut self, delta_time: Duration) -> anyhow::Result<()> {
        let (output, view, mut encoder) = match self.mere_renderer.create_encoder()? {
            Some(res) => res,
            None => return Ok(()),
        };

        let (device, queue) = self.mere_renderer.get_device_queue();

        let materials = self.world.materials();
        self.mere_renderer.render(
            &view,
            &mut encoder,
            self.world.instances(),
            self.world.resources(),
            &materials,
            &mut self.profiler,
            &self.debug,
        );

        {
            self.egui_renderer.begin_frame(&self.window);

            self.profiler.resolve(&mut encoder);

            self.egui_renderer.debug_window(
                &mut self.profiler,
                &mut self.debug,
                device,
                queue,
                &self.window,
                &mut self.world,
                delta_time,
                &mut self.lock_view,
            );

            self.egui_renderer
                .end_frame_and_draw(device, queue, &mut encoder, &self.window, &view);
        }

        let submission_index = queue.submit(Some(encoder.finish()));
        self.profiler.set_submission_index(submission_index);

        output.present();

        Ok(())
    }

    pub fn after_render(&mut self) {
        self.profiler.finish_frame();
    }
}
