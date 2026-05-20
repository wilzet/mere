use crate::util::{pack_11_11_10, pack_16_16};
use mere_math::{Vec2, Vec3};
use meshopt::VertexDataAdapter;
use std::collections::HashMap;

#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Debug, Default)]
#[repr(C)]
pub struct FullVertex {
    pub vertex: Vertex,
    pub attributes: VertexAttributes,
}

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

pub fn process_vertices(
    vertices: impl IntoIterator<Item = FullVertex>,
) -> (Vec<Vertex>, Vec<VertexAttributes>, Vec<u32>) {
    let (vertices, indices) = weld_and_remap_vertices(vertices);
    let (vertex_positions, vertex_attributes) = split_full_vertex(vertices);

    (vertex_positions, vertex_attributes, indices)
}

pub fn split_full_vertex(
    vertices: impl IntoIterator<Item = FullVertex>,
) -> (Vec<Vertex>, Vec<VertexAttributes>) {
    vertices
        .into_iter()
        .map(|v| (v.vertex, v.attributes))
        .unzip::<_, _, Vec<_>, Vec<_>>()
}

fn weld_and_remap_vertices(
    vertices: impl IntoIterator<Item = FullVertex>,
) -> (Vec<FullVertex>, Vec<u32>) {
    let input_vertices = vertices.into_iter().collect::<Vec<_>>();
    let input_vertices_count = input_vertices.len();

    // Weld by position
    let mut position_to_welded_id = HashMap::with_capacity(input_vertices_count);
    let mut unique_vertices = Vec::new();

    let position_remap = input_vertices
        .into_iter()
        .map(|v| {
            let key = [
                v.vertex.position.x.to_bits(),
                v.vertex.position.y.to_bits(),
                v.vertex.position.z.to_bits(),
                v.attributes.normal,
                v.attributes.uv,
            ];

            *position_to_welded_id.entry(key).or_insert_with(|| {
                let next = unique_vertices.len() as u32;
                unique_vertices.push(v.clone());
                next
            })
        })
        .collect::<Vec<_>>();

    let indices = (0..input_vertices_count)
        .map(|i| position_remap[i as usize])
        .collect::<Vec<_>>();

    (unique_vertices, indices)
}
