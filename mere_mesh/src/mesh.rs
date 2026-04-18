use crate::{Vertex, util::*};
use mere_math::{Vec2, Vec3};
use std::{
    fs,
    os::raw::{c_uint, c_void},
    path::Path,
};

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

    pub fn from_gltf_primitive(
        primitive: gltf::Primitive,
        buffers: &Vec<gltf::buffer::Data>,
    ) -> MereMesh {
        let reader = primitive.reader(|b| Some(&buffers[b.index()]));

        let indices = reader
            .read_indices()
            .unwrap()
            .into_u32()
            .collect::<Vec<_>>();

        let (positions, normals, tex_coords, tangents) = match reader.read_tangents() {
            Some(tangents) => {
                let positions = reader
                    .read_positions()
                    .unwrap()
                    .map(Vec3::from)
                    .collect::<Vec<_>>();
                let normals = reader.read_normals().map_or_else(
                    || vec![pack_11_11_10(Vec3::ZERO); positions.len()],
                    |it| it.map(|n| pack_11_11_10(Vec3::from(n))).collect(),
                );
                let tex_coords = reader.read_tex_coords(0).map_or_else(
                    || vec![pack_16_16(Vec2::ZERO); positions.len()],
                    |it| it.into_f32().map(|t| pack_16_16(Vec2::from(t))).collect(),
                );
                let tangents = tangents
                    .map(|t| {
                        pack_10_10_10_2(
                            Vec3::new(t[0], t[1], t[2]),
                            ((t[3].signum() as i32 + 1) >> 1) as u32,
                        )
                    })
                    .collect::<Vec<_>>();

                (positions, normals, tex_coords, tangents)
            }
            None => {
                let positions = reader
                    .read_positions()
                    .unwrap()
                    .map(Vec3::from)
                    .collect::<Vec<_>>();
                let normals = reader.read_normals().map_or_else(
                    || vec![Vec3::ZERO; positions.len()],
                    |it| it.map(Vec3::from).collect(),
                );
                let tex_coords = reader.read_tex_coords(0).map_or_else(
                    || vec![Vec2::ZERO; positions.len()],
                    |it| it.into_f32().map(Vec2::from).collect(),
                );
                let tangents = calculate_tangents(&indices, &positions, &normals, &tex_coords);

                let normals = normals.into_iter().map(pack_11_11_10).collect::<Vec<_>>();
                let tex_coords = tex_coords.into_iter().map(pack_16_16).collect::<Vec<_>>();

                (positions, normals, tex_coords, tangents)
            }
        };

        let index_count = indices.len();
        let vertices = indices.into_iter().map(|i| {
            let index = i as usize;
            Vertex {
                position: positions[index],
                normal: normals[index],
                tex_coord: tex_coords[index],
                tangent: tangents[index],
            }
        });

        Self::new(vertices, index_count)
    }
}

pub fn write_mere_file(output_path: &Path, meshes: Vec<MereMesh>) -> anyhow::Result<()> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mesh_count = meshes.len();
    let mut bin = (mesh_count as u32).to_le_bytes().to_vec();
    bin.extend_from_slice(
        meshes
            .iter()
            .flat_map(|m| m.into_mere_file())
            .collect::<Vec<_>>()
            .as_slice(),
    );

    fs::write(&output_path, bin)?;

    Ok(())
}

pub fn read_mere_file(path: &Path) -> anyhow::Result<Vec<MereMesh>> {
    let mere_bytes = fs::read(&path)?;
    let mesh_count = u32::from_le_bytes(mere_bytes[0..4].try_into()?) as usize;

    let mut offset = 4;
    let mut meshes = Vec::new();
    for _ in 0..mesh_count {
        let (mesh, read_bytes) = MereMesh::from_mere_file(&mere_bytes[offset..])?;
        meshes.push(mesh);
        offset += read_bytes;
    }

    Ok(meshes)
}
