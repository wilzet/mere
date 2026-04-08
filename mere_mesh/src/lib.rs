use std::os::raw::{c_uint, c_void};

mod vertex;

pub use vertex::Vertex;

#[derive(Clone, Debug, Default)]
pub struct MereMesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl MereMesh {
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
                size_of::<Vertex>(),
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

    pub fn into_mere_file(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        // Write header
        bytes.extend_from_slice(&(self.vertices.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&(self.indices.len() as u64).to_le_bytes());

        // Write mesh data
        let vertex_bytes = bytemuck::cast_slice(&self.vertices);
        bytes.extend_from_slice(vertex_bytes);

        let index_bytes = bytemuck::cast_slice(&self.indices);
        bytes.extend_from_slice(index_bytes);

        bytes
    }

    pub fn from_mere_file<'a>(bytes: &[u8]) -> anyhow::Result<(Self, usize)> {
        if bytes.len() < 16 {
            anyhow::bail!("File too small to contain header");
        }

        // Read Header
        let v_len = u64::from_le_bytes(bytes[0..8].try_into()?) as usize;
        let i_len = u64::from_le_bytes(bytes[8..16].try_into()?) as usize;
        let offset = 16;

        // Read mesh data
        let v_end = offset + v_len * std::mem::size_of::<Vertex>();
        let i_end = v_end + i_len * std::mem::size_of::<u32>();

        if bytes.len() < i_end {
            anyhow::bail!(
                "File truncated: expected {} bytes, got {}",
                i_end,
                bytes.len()
            );
        }

        let vertices = bytemuck::cast_slice(&bytes[offset..v_end]);
        let indices = bytemuck::cast_slice(&bytes[v_end..i_end]);

        Ok((
            MereMesh {
                vertices: vertices.to_vec(),
                indices: indices.to_vec(),
            },
            i_end,
        ))
    }
}
