use mere_math::Vec3;

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub fov: f32,
    pub aspect_ratio: f32,
    pub near: f32,
    pub far: f32,
}

#[derive(Clone, Copy, Debug)]
pub enum LightKind {
    Directional { direction: Vec3 },
    Point { range: f32 },
}

#[derive(Clone, Copy, Debug)]
pub struct Light {
    pub kind: LightKind,
    pub color: Vec3,
    pub intensity: f32,
}
