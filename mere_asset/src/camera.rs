use mere_math::{Mat4, Quat, Transform, Vec3};

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub(crate) fov_y: f32,
    pub(crate) aspect: f32,
    pub(crate) near: f32,
    pub(crate) far: f32,
    pub(crate) transform: Transform,
}

impl Camera {
    pub fn new(fov_y: f32, aspect: f32, near: f32, far: f32, position: Vec3) -> Self {
        Self {
            fov_y,
            aspect,
            near,
            far,
            transform: Transform::from_translation(position),
        }
    }

    pub fn look_at(&mut self, target: Vec3) {
        let direction = (target - self.transform.translation).normalize();
        self.transform.rotation = Quat::look_to_rh(direction, self.transform.up());
    }

    pub fn transform(&mut self) -> &mut Transform {
        &mut self.transform
    }

    pub fn view_matrix(&self) -> Mat4 {
        self.transform.inverse()
    }

    pub fn projection_matrix(&self) -> Mat4 {
        Mat4::perspective_rh(self.fov_y, self.aspect, self.near, self.far)
    }
}
