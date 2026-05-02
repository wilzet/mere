use mere_math::Vec3;

#[repr(C)]
#[derive(Clone, Copy, Default, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Meshlet {
    pub vertex_offset: u32,
    pub vertex_count: u32,
    pub index_offset: u32,
    pub index_count: u32,
    // pub lod: u32,
    pub bounds: BoundingSphere,
    pub parent_bounds: BoundingSphere,
    // pub error: f32,
    // pub parent_error: f32,
}

impl Meshlet {
    pub const MAX_VERTICES: usize = 64;
    pub const MAX_TRIANGLES: usize = 128;
    pub const MIN_TRIANGLES: usize = (Self::MAX_TRIANGLES / 3) & !3;
    pub const MAX_INDICES_PER_MESHLET: u32 = Self::MAX_TRIANGLES as u32 * 3;
    pub const FILL_WEIGHT: f32 = 2.0;
}

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct BoundingSphere {
    pub center: Vec3,
    pub radius: f32,
}

unsafe impl bytemuck::Zeroable for BoundingSphere {}
unsafe impl bytemuck::Pod for BoundingSphere {}

impl BoundingSphere {
    pub fn new(center: Vec3, radius: f32) -> Self {
        Self { center, radius }
    }
}
