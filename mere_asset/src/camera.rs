use mere_math::{Mat4, Transform};

#[derive(Clone, Copy, Debug, Default)]
pub struct Camera {
    pub(crate) fov_y: f32,
    pub(crate) aspect: f32,
    pub(crate) near: f32,
    pub(crate) far: f32,
    pub(crate) transform: Transform,
}

impl Camera {
    pub fn view_matrix(&self) -> Mat4 {
        self.transform.inverse()
    }

    pub fn projection_matrix(&self) -> Mat4 {
        Mat4::perspective_rh(self.fov_y, self.aspect, self.near, self.far)
    }
}
