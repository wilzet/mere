use crate::{
    camera::CameraController,
    egui_render::{DebugWindow, EguiRenderer},
    renderer::Renderer,
};
use mere_asset::{Camera, Material, World};
use mere_math::Vec3;
use std::{sync::Arc, time::Duration};
use winit::{
    event::*,
    event_loop::ActiveEventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};

mod camera;
mod egui_render;
mod lights;
mod pipeline;
mod renderer;

pub const CLUSTER_SLOTS: u32 = 1 << 20;

pub struct State {
    mere_renderer: Renderer,
    egui_renderer: EguiRenderer,
    window: Arc<Window>,
    lock_cursor: bool,
    camera_controller: CameraController,
    stored_cursor_pos: (f64, f64),
    world: World,
}

impl State {
    pub async fn new(window: Arc<Window>) -> anyhow::Result<Self> {
        let (mere_renderer, mut world) = Renderer::new(window.clone()).await?;

        let (device, queue) = mere_renderer.get_device_queue();
        let config = mere_renderer.get_config();

        //world.load_gltf("sponza/main_sponza", device, queue)?;
        //world.load_gltf("sponza/pkg_a_curtains", device, queue)?;
        for x in 0..10 {
            for y in 0..10 {
                let teapot = world.load_gltf("utah_teapot", device, queue)?[0];
                world
                    .get_instance_mut(teapot)
                    .unwrap()
                    .transform
                    .translation += Vec3::X * 6.0 * x as f32 + Vec3::Z * 6.0 * y as f32;
            }
        }

        let mut camera = Camera::new(
            45.0f32.to_radians(),
            config.width as f32 / config.height as f32,
            0.1,
            100.0,
            Vec3::new(1.0, 5.0, -5.0),
        );
        camera.look_at(Vec3::ZERO);
        world.add_camera(camera);

        let camera_controller = CameraController::new(5.0, 0.002);

        let egui_renderer = EguiRenderer::new(&device, config.format, None, 1, &window);

        mere_log::success!("State initialization complete.");

        Ok(Self {
            mere_renderer,
            egui_renderer,
            window,
            lock_cursor: false,
            camera_controller,
            stored_cursor_pos: (0.0, 0.0),
            world,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.mere_renderer.resize(width, height);

            self.world
                .main_camera_mut()
                .resize(width as f32 / height as f32);
        }
    }

    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }

    pub fn handle_input(&mut self, event_loop: &ActiveEventLoop, event: &WindowEvent) {
        let ui_input = self.egui_renderer.handle_input(&self.window, &event);

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
            WindowEvent::CursorMoved { position, .. } => {
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
        self.world.prepare_meshlet_resources(device, queue);

        self.camera_controller
            .update_camera(self.world.main_camera_mut(), dt);
        self.mere_renderer
            .update_main_camera(self.world.main_camera());
        self.mere_renderer.update_light(dt);
    }

    pub fn render(&mut self, delta_time: Duration) -> anyhow::Result<()> {
        let (output, view, mut encoder) = match self.mere_renderer.create_encoder()? {
            Some(res) => res,
            None => return Ok(()),
        };

        let (device, queue) = self.mere_renderer.get_device_queue();
        let config = self.mere_renderer.get_config();

        let material_lock = self.world.get_material(Material::DEFAULT_MATERIAL_ID);
        let material = material_lock.read();

        self.mere_renderer.render(
            &view,
            &mut encoder,
            self.world.instances(),
            self.world.resources(),
            &material,
        );

        {
            self.egui_renderer.begin_frame(&self.window);

            self.egui_renderer
                .debug_window(&self.window, &self.world, delta_time);

            let screen_descriptor = egui_wgpu::ScreenDescriptor {
                size_in_pixels: [config.width, config.height],
                pixels_per_point: self.window.scale_factor() as f32
                    * self.egui_renderer.scale_factor(),
            };

            self.egui_renderer.end_frame_and_draw(
                device,
                queue,
                &mut encoder,
                &self.window,
                &view,
                screen_descriptor,
            );
        }

        queue.submit(Some(encoder.finish()));
        output.present();

        Ok(())
    }
}
