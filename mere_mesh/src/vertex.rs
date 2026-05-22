use crate::util::{pack_11_11_10, pack_16_16};
use mere_math::{Vec2, Vec3};
use meshopt::VertexDataAdapter;

#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct Vertex {
    pub position: Vec3,
}

unsafe impl bytemuck::Pod for Vertex {}
unsafe impl bytemuck::Zeroable for Vertex {}

impl meshopt::DecodePosition for Vertex {
    fn decode_position(&self) -> [f32; 3] {
        self.position.into()
    }
}

impl Vertex {
    pub fn create_vertex_adapter(vertices: &[Vertex]) -> VertexDataAdapter<'_> {
        let position_offset = std::mem::offset_of!(Vertex, position);
        let vertex_stride = size_of::<Vertex>();
        let vertex_data = meshopt::typed_to_bytes(vertices);
        VertexDataAdapter::new(vertex_data, vertex_stride, position_offset).unwrap()
    }
}

#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Debug)]
#[repr(C)]
pub struct VertexAttributes {
    pub normal: u32,
    pub uv: u32,
    pub tangent: u32,
}

impl Default for VertexAttributes {
    fn default() -> Self {
        Self {
            normal: pack_11_11_10(Vec3::ZERO),
            uv: pack_16_16(Vec2::ZERO),
            tangent: 0,
        }
    }
}
