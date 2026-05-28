use mere_asset::Camera;
use mere_math::{Quat, Vec3};
use winit::{dpi::PhysicalPosition, event::MouseScrollDelta, keyboard::KeyCode};

pub struct CameraController {
    speed: f32,
    sensitivity: f32,
    is_forward_pressed: bool,
    is_backward_pressed: bool,
    is_left_pressed: bool,
    is_right_pressed: bool,
    is_up_pressed: bool,
    is_down_pressed: bool,
    rotate_horizontal: f32,
    rotate_vertical: f32,
    scroll: f32,
}

impl CameraController {
    pub const MIN_ZOOM: f32 = 1f32.to_radians();
    pub const MAX_ZOOM: f32 = 170f32.to_radians();

    pub fn new(speed: f32, sensitivity: f32) -> Self {
        Self {
            speed,
            sensitivity,
            is_forward_pressed: false,
            is_backward_pressed: false,
            is_left_pressed: false,
            is_right_pressed: false,
            is_up_pressed: false,
            is_down_pressed: false,
            rotate_horizontal: 0.0,
            rotate_vertical: 0.0,
            scroll: 0.0,
        }
    }

    pub fn handle_key(&mut self, code: KeyCode, is_pressed: bool) -> bool {
        match code {
            KeyCode::KeyW | KeyCode::ArrowUp => {
                self.is_forward_pressed = is_pressed;
                true
            }
            KeyCode::KeyA | KeyCode::ArrowLeft => {
                self.is_left_pressed = is_pressed;
                true
            }
            KeyCode::KeyS | KeyCode::ArrowDown => {
                self.is_backward_pressed = is_pressed;
                true
            }
            KeyCode::KeyD | KeyCode::ArrowRight => {
                self.is_right_pressed = is_pressed;
                true
            }
            KeyCode::KeyE | KeyCode::PageUp => {
                self.is_up_pressed = is_pressed;
                true
            }
            KeyCode::KeyQ | KeyCode::PageDown => {
                self.is_down_pressed = is_pressed;
                true
            }
            _ => false,
        }
    }

    pub fn handle_mouse_move(&mut self, mouse_dx: f64, mouse_dy: f64) {
        self.rotate_horizontal = mouse_dx as f32;
        self.rotate_vertical = mouse_dy as f32;
    }

    pub fn handle_mouse_scroll(&mut self, delta: &MouseScrollDelta) {
        self.scroll = -match delta {
            MouseScrollDelta::LineDelta(_, scroll) => scroll * 100.0,
            MouseScrollDelta::PixelDelta(PhysicalPosition { y: scroll, .. }) => *scroll as f32,
        }
    }

    pub fn update_camera(&mut self, camera: &mut Camera, delta_time: f32) {
        // Move
        let forward = camera.transform.forward();
        let forward_norm = forward.normalize();

        let right = camera.transform.right();
        let right_norm = right.normalize();

        #[rustfmt::skip]
        let forward = (self.is_forward_pressed as i32 - self.is_backward_pressed as i32) as f32 * forward_norm;
        #[rustfmt::skip]
        let right = (self.is_right_pressed as i32 - self.is_left_pressed as i32) as f32 * right_norm;
        let up = (self.is_up_pressed as i32 - self.is_down_pressed as i32) as f32 * Vec3::Y;
        camera.transform.translation +=
            (forward + right + up).normalize_or_zero() * self.speed * delta_time;

        // Zoom
        const ZOOM_WEIGHT: f32 = 0.1;
        camera.fov_y += self.scroll * self.sensitivity * ZOOM_WEIGHT;

        camera.fov_y = camera.fov_y.clamp(Self::MIN_ZOOM, Self::MAX_ZOOM);

        self.scroll = 0.0;

        // Rotate
        let fov_scale = camera.fov_y / 45.0f32.to_radians();
        let effective_sensitivity = self.sensitivity * fov_scale;

        let yaw = Quat::from_rotation_y(-self.rotate_horizontal * effective_sensitivity);
        let pitch = Quat::from_rotation_x(-self.rotate_vertical * effective_sensitivity);
        camera.transform.rotation = yaw * camera.transform.rotation * pitch;

        self.rotate_horizontal = 0.0;
        self.rotate_vertical = 0.0;
    }
}
