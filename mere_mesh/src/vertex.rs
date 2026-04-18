use mere_math::{Vec3};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Vertex {
    pub position: Vec3,
    pub normal: u32,
    pub tex_coord: u32,
    pub tangent: u32,
}

unsafe impl bytemuck::Zeroable for Vertex {}
unsafe impl bytemuck::Pod for Vertex {}

impl meshopt::DecodePosition for Vertex {
    fn decode_position(&self) -> [f32; 3] {
        self.position.into()
    }
}

impl Vertex {
    const ATTRIBUTES: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![0 => Float32x3, 1 => Uint32, 2 => Uint32, 3 => Uint32];

    pub const fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}
