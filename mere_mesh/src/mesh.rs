use crate::{
    meshlet::{Meshlet, build_per_meshlet_attributes, generate_meshlets},
    util::*,
    vertex::{Vertex, VertexAttributes},
};
use mere_math::{Vec2, Vec3};
use std::{fs, path::Path, sync::Arc};

#[derive(Clone, Debug, Default)]
pub struct MeshletMesh {
    pub name: String,
    pub vertex_positions: Arc<[Vertex]>,
    pub vertex_attributes: Arc<[VertexAttributes]>,
    pub meshlet_vertex_indices: Arc<[u32]>,
    pub meshlet_indices: Arc<[u8]>,
    pub meshlets: Arc<[Meshlet]>,
    pub aabb: Aabb,
    pub meshlet_offset: u32,
}

impl MeshletMesh {
    pub fn new(vertices: impl IntoIterator<Item = (Vertex, VertexAttributes)>) -> Self {
        let (vertex_positions, vertex_attributes) =
            vertices.into_iter().unzip::<_, _, Vec<_>, Vec<_>>();

        let aabb = Aabb::from_vertices(&vertex_positions);

        let vertices = Vertex::create_vertex_adapter(&vertex_positions);
        let vertex_positions_remap = meshopt::generate_position_remap(&vertices);
        let indices = (0..vertex_positions.len() as u32).collect::<Vec<_>>();

        let (meshlets, cull_data) =
            generate_meshlets(&vertices, &indices, &vertex_positions_remap, None);
        // let mut simplification_queue = (0..meshlets.len() as u32).collect::<Vec<_>>();

        let mut meshlet_attributes = Vec::new();
        let mut meshlet_meshlets = Vec::new();

        for (i, (meshlet, &cull_data)) in meshlets.meshlets.iter().zip(cull_data.iter()).enumerate()
        {
            build_per_meshlet_attributes(
                meshlet,
                cull_data,
                meshlets.get(i).vertices,
                &vertex_attributes,
                &mut meshlet_attributes,
                &mut meshlet_meshlets,
            );
        }

        Self {
            name: "".to_string(),
            vertex_positions: vertex_positions.into(),
            vertex_attributes: meshlet_attributes.into(),
            meshlet_vertex_indices: meshlets.vertices.into(),
            meshlet_indices: meshlets.triangles.into(),
            meshlets: meshlet_meshlets.into(),
            aabb,
            meshlet_offset: 0,
        }
    }

    pub fn into_mere_file(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        // Write header
        bytes.extend_from_slice(&(self.vertex_positions.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&(self.vertex_attributes.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&(self.meshlet_vertex_indices.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&(self.meshlet_indices.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&(self.meshlets.len() as u64).to_le_bytes());

        // Write mesh data
        let position_bytes = bytemuck::cast_slice(&self.vertex_positions);
        bytes.extend_from_slice(position_bytes);

        let attribute_bytes = bytemuck::cast_slice(&self.vertex_attributes);
        bytes.extend_from_slice(attribute_bytes);

        let meshlet_vertex_index_bytes = bytemuck::cast_slice(&self.meshlet_vertex_indices);
        bytes.extend_from_slice(meshlet_vertex_index_bytes);

        let meshlet_index_bytes = bytemuck::cast_slice(&self.meshlet_indices);
        bytes.extend_from_slice(meshlet_index_bytes);

        let meshlet_bytes = bytemuck::cast_slice(&self.meshlets);
        bytes.extend_from_slice(meshlet_bytes);

        bytes
    }

    pub fn from_mere_file<'a>(bytes: &[u8]) -> anyhow::Result<(Self, usize)> {
        if bytes.len() < 40 {
            anyhow::bail!("File too small to contain header");
        }

        // Read Header
        let v_len = u64::from_le_bytes(bytes[0..8].try_into()?) as usize;
        let attr_len = u64::from_le_bytes(bytes[8..16].try_into()?) as usize;
        let m_v_i_len = u64::from_le_bytes(bytes[16..24].try_into()?) as usize;
        let m_i_len = u64::from_le_bytes(bytes[24..32].try_into()?) as usize;
        let m_len = u64::from_le_bytes(bytes[32..40].try_into()?) as usize;
        let offset = 40;

        // Read mesh data
        let v_end = offset + v_len * size_of::<Vertex>();
        let attr_end = v_end + attr_len * size_of::<VertexAttributes>();
        let m_v_i_end = attr_end + m_v_i_len * size_of::<u32>();
        let m_i_end = m_v_i_end + m_i_len * size_of::<u8>();
        let m_end = m_i_end + m_len * size_of::<Meshlet>();

        if bytes.len() < m_end {
            anyhow::bail!(
                "File truncated: expected {} bytes, got {}",
                m_end,
                bytes.len()
            );
        }

        let vertex_positions = Arc::from(bytemuck::pod_collect_to_vec(&bytes[offset..v_end]));
        let vertex_attributes = Arc::from(bytemuck::pod_collect_to_vec(&bytes[v_end..attr_end]));
        let meshlet_vertex_indices =
            Arc::from(bytemuck::pod_collect_to_vec(&bytes[attr_end..m_v_i_end]));
        let meshlet_indices = Arc::from(bytemuck::pod_collect_to_vec(&bytes[m_v_i_end..m_i_end]));
        let meshlets = Arc::from(bytemuck::pod_collect_to_vec(&bytes[m_i_end..m_end]));

        let aabb = Aabb::from_vertices(&vertex_positions);

        Ok((
            Self {
                name: "".to_string(),
                vertex_positions,
                vertex_attributes,
                meshlet_vertex_indices,
                meshlet_indices,
                meshlets,
                aabb,
                meshlet_offset: 0,
            },
            m_end,
        ))
    }

    pub fn from_gltf_primitive(
        primitive: gltf::Primitive,
        buffers: &Vec<gltf::buffer::Data>,
    ) -> Self {
        let reader = primitive.reader(|b| Some(&buffers[b.index()]));

        let indices = reader
            .read_indices()
            .unwrap()
            .into_u32()
            .collect::<Vec<_>>();

        let (positions, normals, uvs, tangents) = match reader.read_tangents() {
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
                let uvs = reader.read_tex_coords(0).map_or_else(
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

                (positions, normals, uvs, tangents)
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
                let uvs = reader.read_tex_coords(0).map_or_else(
                    || vec![Vec2::ZERO; positions.len()],
                    |it| it.into_f32().map(Vec2::from).collect(),
                );
                let tangents = calculate_tangents(&indices, &positions, &normals, &uvs);

                let normals = normals.into_iter().map(pack_11_11_10).collect::<Vec<_>>();
                let uvs = uvs.into_iter().map(pack_16_16).collect::<Vec<_>>();

                (positions, normals, uvs, tangents)
            }
        };

        let vertices = indices.into_iter().map(|i| {
            let index = i as usize;
            (
                Vertex {
                    position: positions[index],
                },
                VertexAttributes {
                    normal: normals[index],
                    uv: uvs[index],
                    tangent: tangents[index],
                },
            )
        });

        Self::new(vertices)
    }

    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }
}

pub fn write_mere_file(output_path: &Path, meshes: Vec<MeshletMesh>) -> anyhow::Result<()> {
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

pub fn read_mere_file(path: &Path) -> anyhow::Result<Vec<MeshletMesh>> {
    let mere_bytes = fs::read(&path)?;
    let mesh_count = u32::from_le_bytes(mere_bytes[0..4].try_into()?) as usize;

    let mut offset = 4;
    let mut meshes = Vec::new();
    for _ in 0..mesh_count {
        let (mesh, read_bytes) = MeshletMesh::from_mere_file(&mere_bytes[offset..])?;
        meshes.push(mesh);
        offset += read_bytes;
    }

    Ok(meshes)
}

#[derive(Clone, Copy, Default, Debug)]
#[repr(C)]
pub struct Aabb {
    center: Vec3,
    half_extents: Vec3,
}

unsafe impl bytemuck::Zeroable for Aabb {}
unsafe impl bytemuck::Pod for Aabb {}

impl Aabb {
    pub fn new(center: Vec3, half_extents: Vec3) -> Self {
        Self {
            center,
            half_extents,
        }
    }

    pub fn from_vertices(vertices: &[impl meshopt::DecodePosition]) -> Self {
        let (min, max) = vertices
            .iter()
            .fold((Vec3::MAX, Vec3::MIN), |(min, max), v| {
                (
                    min.min(v.decode_position().into()),
                    max.max(v.decode_position().into()),
                )
            });

        let center = (min + max) * 0.5;
        let half_extents = max - center;

        Self {
            center,
            half_extents,
        }
    }
}
