use mere_math::Vec3;

#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Debug, Default)]
#[repr(C)]
pub struct FullVertex {
    pub position: Vertex,
    pub attributes: VertexAttributes,
}

#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct Vertex {
    pub position: Vec3,
}

unsafe impl bytemuck::Pod for Vertex {}
unsafe impl bytemuck::Zeroable for Vertex {}

#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Debug, Default)]
#[repr(C)]
pub struct VertexAttributes {
    pub normal: u32,
    pub uv: u32,
    pub tangent: u32,
}

impl meshopt::DecodePosition for Vertex {
    fn decode_position(&self) -> [f32; 3] {
        self.position.into()
    }
}

pub fn split_full_vertex(
    vertices: impl IntoIterator<Item = FullVertex>,
) -> (Vec<Vertex>, Vec<VertexAttributes>) {
    vertices
        .into_iter()
        .map(|v| (v.position, v.attributes))
        .unzip::<_, _, Vec<_>, Vec<_>>()
}
