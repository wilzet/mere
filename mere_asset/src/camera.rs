use mere_math::{Mat4, Quat, Transform, Vec3};

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub transform: Transform,
    // Field of view in radians.
    pub fov_y: f32,
    aspect: f32,
    near: f32,
    far: f32,
}

impl Camera {
    pub fn new(fov_y_radians: f32, aspect: f32, near: f32, far: f32, position: Vec3) -> Self {
        Self {
            transform: Transform::from_translation(position),
            fov_y: fov_y_radians,
            aspect,
            near,
            far,
        }
    }

    pub fn look_at(&mut self, target: Vec3) {
        self.transform.rotation =
            Quat::look_at_rh(self.transform.translation, target, self.transform.up()).conjugate();
    }

    pub fn resize(&mut self, aspect: f32) {
        self.aspect = aspect;
    }

    pub fn view_matrix(&self) -> Mat4 {
        self.transform.inverse()
    }

    pub fn projection_matrix(&self) -> Mat4 {
        Mat4::perspective_rh(self.fov_y, self.aspect, self.near, self.far)
    }
}
