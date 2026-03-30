use mere_asset::Camera;
use mere_math::Mat4;
use winit::keyboard::KeyCode;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Debug)]
pub(crate) struct CameraUniform {
    view_proj: [[f32; 4]; 4],
}

impl CameraUniform {
    pub fn new() -> Self {
        Self {
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
        }
    }

    pub fn update_view_proj(&mut self, camera: &Camera) {
        self.view_proj = (camera.projection_matrix() * camera.view_matrix()).to_cols_array_2d();
    }
}

pub(crate) struct CameraController {
    speed: f32,
    is_forward_pressed: bool,
    is_backward_pressed: bool,
    is_left_pressed: bool,
    is_right_pressed: bool,
}

impl CameraController {
    pub fn new(speed: f32) -> Self {
        Self {
            speed,
            is_forward_pressed: false,
            is_backward_pressed: false,
            is_left_pressed: false,
            is_right_pressed: false,
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
            _ => false,
        }
    }

    pub fn update_camera(&self, camera: &mut Camera) {
        let forward = camera.transform().forward();
        let forward_norm = forward.normalize();

        let right = camera.transform().right();
        let right_norm = right.normalize();

        if self.is_forward_pressed {
            camera.transform().translation += forward_norm * self.speed;
        }
        if self.is_backward_pressed {
            camera.transform().translation -= forward_norm * self.speed;
        }

        if self.is_right_pressed {
            camera.transform().translation += right_norm * self.speed;
        }
        if self.is_left_pressed {
            camera.transform().translation -= right_norm * self.speed;
        }
    }
}
