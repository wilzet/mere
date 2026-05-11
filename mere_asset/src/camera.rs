use mere_math::{Mat4, Quat, Transform, Vec3, Vec4};

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub transform: Transform,
    // Field of view in radians.
    pub fov_y: f32,
    pub viewport: Vec4,
    aspect: f32,
    near: f32,
    far: f32,
}

impl Camera {
    pub fn new(
        fov_y_radians: f32,
        width: u32,
        height: u32,
        near: f32,
        far: f32,
        position: Vec3,
    ) -> Self {
        let aspect = width as f32 / height as f32;
        Self {
            transform: Transform::from_translation(position),
            fov_y: fov_y_radians,
            viewport: Vec4::new(0.0, 0.0, width as f32, height as f32),
            aspect,
            near,
            far,
        }
    }

    pub fn look_at(&mut self, target: Vec3) {
        self.transform.rotation =
            Quat::look_at_rh(self.transform.translation, target, self.transform.up()).conjugate();
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        let aspect = width as f32 / height as f32;

        self.aspect = aspect;
        self.viewport = Vec4::new(0.0, 0.0, width as f32, height as f32);
    }

    pub fn view_matrix(&self) -> Mat4 {
        self.transform.inverse()
    }

    pub fn projection_matrix(&self) -> Mat4 {
        Mat4::perspective_infinite_reverse_rh(self.fov_y, self.aspect, self.near)
    }

    pub fn frustum_matrix(&self) -> Mat4 {
        Mat4::perspective_rh(self.fov_y, self.aspect, self.near, self.far) * self.view_matrix()
    }
}
