use crate::{
    meshlet::{BoundingSphere, Meshlet},
    util::*,
    vertex::{FullVertex, Vertex, VertexAttributes, split_full_vertex},
};
use mere_math::{Vec2, Vec3};
use meshopt::VertexDataAdapter;
use std::{fs, mem, path::Path, sync::Arc};

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
    pub fn new(vertices: impl IntoIterator<Item = FullVertex>) -> Self {
        let vertices = vertices.into_iter().collect::<Vec<_>>();
        let (vertex_count, vertex_remap) = meshopt::generate_vertex_remap(&vertices, None);

        let indices = meshopt::remap_index_buffer(None, vertices.len(), &vertex_remap);
        let vertices = meshopt::remap_vertex_buffer(&vertices, vertex_count, &vertex_remap);

        let (vertex_positions, vertex_attributes) = split_full_vertex(vertices);

        let aabb = Aabb::from_vertices(&vertex_positions);

        let (meshlet_vertex_indices, meshlet_indices, meshlets) =
            Self::generate_meshlets(&vertex_positions, &indices);

        Self {
            name: "".to_string(),
            vertex_positions: vertex_positions.into(),
            vertex_attributes: vertex_attributes.into(),
            meshlet_vertex_indices: meshlet_vertex_indices.into(),
            meshlet_indices: meshlet_indices.into(),
            meshlets: meshlets.into(),
            aabb,
            meshlet_offset: 0,
        }
    }

    fn generate_meshlets(
        vertices: &[Vertex],
        indices: &[u32],
    ) -> (Vec<u32>, Vec<u8>, Vec<Meshlet>) {
        let vertex_adapter = &Self::create_vertex_adapter(&vertices);
        let meshlets = meshopt::build_meshlets_spatial(
            &indices,
            vertex_adapter,
            Meshlet::MAX_VERTICES,
            Meshlet::MIN_TRIANGLES,
            Meshlet::MAX_TRIANGLES,
            Meshlet::FILL_WEIGHT,
        );

        let bounds = meshlets.iter().map(|m| {
            let bounds = meshopt::compute_meshlet_bounds(m, vertex_adapter);
            BoundingSphere::new(bounds.center.into(), bounds.radius)
        });

        let mere_meshlets = meshlets
            .meshlets
            .iter()
            .zip(bounds)
            .map(|(meshlet, bounds)| Meshlet {
                vertex_offset: meshlet.vertex_offset,
                vertex_count: meshlet.vertex_count,
                index_offset: meshlet.triangle_offset,
                index_count: meshlet.triangle_count * 3,
                bounds,
                parent_bounds: BoundingSphere::new(bounds.center, f32::NEG_INFINITY),
            })
            .collect();

        (meshlets.vertices, meshlets.triangles, mere_meshlets)
    }

    fn create_vertex_adapter(vertices: &[Vertex]) -> VertexDataAdapter<'_> {
        let position_offset = mem::offset_of!(Vertex, position);
        let vertex_stride = size_of::<Vertex>();
        let vertex_data = meshopt::typed_to_bytes(vertices);
        VertexDataAdapter::new(vertex_data, vertex_stride, position_offset).unwrap()
    }

    pub fn into_mere_file(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        // Write header
        bytes.extend_from_slice(&(self.vertex_positions.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&(self.meshlet_vertex_indices.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&(self.meshlet_indices.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&(self.meshlets.len() as u64).to_le_bytes());

        let full_vertices = self
            .vertex_positions
            .into_iter()
            .zip(self.vertex_attributes.into_iter())
            .map(|(&p, &a)| FullVertex {
                position: p,
                attributes: a,
            })
            .collect::<Vec<_>>();

        // Write mesh data
        let vertex_bytes = bytemuck::cast_slice(&full_vertices);
        bytes.extend_from_slice(vertex_bytes);

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
        let m_v_i_len = u64::from_le_bytes(bytes[8..16].try_into()?) as usize;
        let m_i_len = u64::from_le_bytes(bytes[16..24].try_into()?) as usize;
        let m_len = u64::from_le_bytes(bytes[24..32].try_into()?) as usize;
        let offset = 32;

        // Read mesh data
        let v_end = offset + v_len * size_of::<FullVertex>();
        let m_v_i_end = v_end + m_v_i_len * size_of::<u32>();
        let m_i_end = m_v_i_end + m_i_len * size_of::<u8>();
        let m_end = m_i_end + m_len * size_of::<Meshlet>();

        if bytes.len() < m_end {
            anyhow::bail!(
                "File truncated: expected {} bytes, got {}",
                m_end,
                bytes.len()
            );
        }

        let full_vertices = bytemuck::pod_collect_to_vec(&bytes[offset..v_end]);
        let meshlet_vertex_indices =
            Arc::from(bytemuck::pod_collect_to_vec(&bytes[v_end..m_v_i_end]));
        let meshlet_indices = Arc::from(bytemuck::pod_collect_to_vec(&bytes[m_v_i_end..m_i_end]));
        let meshlets = Arc::from(bytemuck::pod_collect_to_vec(&bytes[m_i_end..m_end]));

        let (vertices, vertex_attributes) = split_full_vertex(full_vertices);

        let aabb = Aabb::from_vertices(&vertices);

        Ok((
            Self {
                name: "".to_string(),
                vertex_positions: vertices.into(),
                vertex_attributes: vertex_attributes.into(),
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
            FullVertex {
                position: Vertex {
                    position: positions[index],
                },
                attributes: VertexAttributes {
                    normal: normals[index],
                    uv: uvs[index],
                    tangent: tangents[index],
                },
            }
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

    pub fn from_vertices(vertices: &[Vertex]) -> Self {
        let (min, max) = vertices
            .iter()
            .fold((Vec3::MAX, Vec3::MIN), |(min, max), v| {
                (min.min(v.position), max.max(v.position))
            });

        let center = (min + max) * 0.5;
        let half_extents = max - center;

        Self {
            center,
            half_extents,
        }
    }
}
