use std::os::raw::{c_uint, c_void};

mod vertex;

pub use vertex::Vertex;

pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl Mesh {
    pub fn new(vertices: impl IntoIterator<Item = Vertex>, index_count: usize) -> Self {
        let vertices = vertices.into_iter().collect::<Vec<_>>();
        let (vertex_count, vertex_remap) = meshopt::generate_vertex_remap(&vertices, None);

        let mesh = Self {
            vertices: vec![Vertex::default(); vertex_count],
            indices: vec![0; index_count],
        };

        unsafe {
            meshopt::ffi::meshopt_remapIndexBuffer(
                mesh.indices.as_ptr() as *mut c_uint,
                std::ptr::null(),
                index_count,
                vertex_remap.as_ptr() as *const c_uint,
            );
        }

        unsafe {
            meshopt::ffi::meshopt_remapVertexBuffer(
                mesh.vertices.as_ptr() as *mut c_void,
                vertices.as_ptr() as *const c_void,
                index_count,
                size_of::<meshopt::Vertex>(),
                vertex_remap.as_ptr() as *const c_uint,
            );
        }

        mesh
    }

    pub fn optimize_mesh(&mut self) {
        let indices = &mut self.indices;
        let vertices = &mut self.vertices;
        meshopt::optimize_vertex_cache_in_place(indices, vertices.len());
        let new_len = meshopt::optimize_vertex_fetch_in_place(indices, vertices);
        self.vertices.resize_with(new_len, || Vertex::default());
    }
}
