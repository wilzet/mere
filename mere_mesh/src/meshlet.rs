use crate::Vertex;
use itertools::Itertools;
use mere_math::Vec3;
use metis::{Graph, option::Opt};
use std::collections::HashMap;

#[repr(C)]
#[derive(Clone, Copy, Default, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Meshlet {
    pub vertex_offset: u32,
    pub vertex_count: u32,
    pub index_offset: u32,
    pub index_count: u32,
    // pub lod: u32,
    pub bounds: BoundingSphere,
    pub parent_error: BoundingSphere,
    pub error: f32,
}

impl Meshlet {
    pub const MAX_VERTICES: usize = 255;
    pub const MAX_TRIANGLES: usize = 128;
    pub const MIN_TRIANGLES: usize = (Self::MAX_TRIANGLES / 3) & !3;
    pub const MAX_INDICES_PER_MESHLET: u32 = Self::MAX_TRIANGLES as u32 * 3;
    pub const FILL_WEIGHT: f32 = 1.0;
}

#[derive(Clone, Default, Debug)]
pub struct MeshletLod {
    pub vertices: Vec<u32>,
    pub indices: Vec<u8>,
    pub meshlets: Vec<Meshlet>,
}

impl MeshletLod {
    pub const TARGET_GROUP_SIZE: usize = 4;
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

fn triangle_center(vertices: &[Vertex], indices: &[u32], tri: usize) -> Vec3 {
    let i = tri * 3;

    let p0 = vertices[indices[i] as usize].position;
    let p1 = vertices[indices[i + 1] as usize].position;
    let p2 = vertices[indices[i + 2] as usize].position;

    (p0 + p1 + p2) * 0.3333333
}

fn triangle_normal(vertices: &[Vertex], indices: &[u32], tri: usize) -> Vec3 {
    let i = tri * 3;

    let p0 = vertices[indices[i] as usize].position;
    let p1 = vertices[indices[i + 1] as usize].position;
    let p2 = vertices[indices[i + 2] as usize].position;

    (p1 - p0).cross(p2 - p0).normalize()
}

pub fn generate_meshlets(
    vertices: &[Vertex],
    indices: &[u32],
    parent_lod: Option<(BoundingSphere, f32)>,
) -> MeshletLod {
    if indices.is_empty() || indices.len() % 3 != 0 {
        return MeshletLod::default();
    }

    let triangle_count = indices.len() / 3;

    // Weld by position for stable topological adjacency.
    let mut position_to_welded_id = HashMap::with_capacity(vertices.len());
    let welded_vertex_remap = vertices
        .iter()
        .map(|vertex| {
            let key = [
                vertex.position.x.to_bits(),
                vertex.position.y.to_bits(),
                vertex.position.z.to_bits(),
            ];

            let next = position_to_welded_id.len() as u32;
            *position_to_welded_id.entry(key).or_insert(next)
        })
        .collect::<Vec<_>>();

    // Vertex -> triangle adjacency.
    let mut vertices_to_triangles = vec![Vec::new(); position_to_welded_id.len()];
    for (i, &index) in indices.iter().enumerate() {
        vertices_to_triangles[welded_vertex_remap[index as usize] as usize].push(i / 3);
    }

    let triangle_centers = (0..triangle_count)
        .map(|i| triangle_center(vertices, indices, i))
        .collect::<Vec<_>>();

    let triangle_normals = (0..triangle_count)
        .map(|i| triangle_normal(vertices, indices, i))
        .collect::<Vec<_>>();

    // Count shared welded vertices per triangle pair.
    let mut pair_shared_count = HashMap::with_capacity(triangle_count * 2);
    for tri_ids in vertices_to_triangles {
        for (a, b) in tri_ids.into_iter().tuple_combinations() {
            let pair = (a.min(b), a.max(b));

            *pair_shared_count.entry(pair).or_insert(0) += 1;
        }
    }

    // METIS weighted adjacency graph.
    let mut adjacency = vec![Vec::new(); triangle_count];
    for ((a, b), shared) in pair_shared_count {
        // Ignore weak vertex-only connectivity.
        if shared < 2 {
            continue;
        }

        let dist = (triangle_centers[a] - triangle_centers[b]).length();
        let normal_similarity = triangle_normals[a].dot(triangle_normals[b]).max(0.0);
        let spatial = 1.0 / (1.0 + dist);

        // Prefer compact + coplanar regions.
        let weight = ((normal_similarity * 0.7 + spatial * 0.3) * 4096.0).clamp(1.0, 4096.0) as i32;

        adjacency[a].push((b, weight));
        adjacency[b].push((a, weight));
    }

    // Convert to CSR format for METIS.
    let total_edges: usize = adjacency.iter().map(Vec::len).sum();

    let mut xadj = Vec::with_capacity(triangle_count + 1);
    let mut adjncy = Vec::with_capacity(total_edges);
    let mut adjwgt = Vec::with_capacity(total_edges);

    for neighbors in &adjacency {
        xadj.push(adjncy.len() as i32);

        for &(id, weight) in neighbors {
            adjncy.push(id as i32);
            adjwgt.push(weight);
        }
    }
    xadj.push(adjncy.len() as i32);

    // Smaller recursive partitions produce rounder meshlets.
    let target_size = Meshlet::MAX_TRIANGLES - 8;
    let partition_count = triangle_count.div_ceil(target_size).max(1);

    let mut options = [-1; metis::NOPTIONS];
    options[metis::option::Seed::INDEX] = 0x5EAF00D;
    options[metis::option::UFactor::INDEX] = 1;
    options[metis::option::MinConn::INDEX] = 1;
    options[metis::option::Contig::INDEX] = 1;
    options[metis::option::ObjType::INDEX] = 0;

    let mut partition_per_triangle = vec![0; triangle_count];
    Graph::new(1, partition_count as i32, &xadj, &adjncy)
        .unwrap()
        .set_adjwgt(&adjwgt)
        .set_options(&options)
        .part_recursive(&mut partition_per_triangle)
        .unwrap();

    let mut partitions = vec![Vec::new(); partition_count];
    for (tri, &partition) in partition_per_triangle.iter().enumerate() {
        partitions[partition as usize].push(tri);
    }

    let vertex_adapter = Vertex::create_vertex_adapter(vertices);

    let mut meshlet_vertices = Vec::new();
    let mut meshlet_indices = Vec::new();
    let mut meshlets = Vec::with_capacity(partitions.len());

    let mut cluster_indices = Vec::new();
    for partition in partitions {
        cluster_indices.clear();
        cluster_indices.reserve(partition.len() * 3);

        for tri in partition {
            let i = tri * 3;

            cluster_indices.extend_from_slice(&indices[i..i + 3]);
        }

        let built = meshopt::build_meshlets_spatial(
            &cluster_indices,
            &vertex_adapter,
            Meshlet::MAX_VERTICES,
            Meshlet::MIN_TRIANGLES,
            Meshlet::MAX_TRIANGLES,
            Meshlet::FILL_WEIGHT,
        );

        let bounds_iter = built.iter().map(|m| {
            let bounds = meshopt::compute_meshlet_bounds(m, &vertex_adapter);
            BoundingSphere::new(bounds.center.into(), bounds.radius)
        });

        for (meshlet, bounds) in built.meshlets.iter().zip(bounds_iter) {
            let vertex_offset = meshlet_vertices.len() as u32;
            let index_offset = meshlet_indices.len() as u32;

            let v0 = meshlet.vertex_offset as usize;
            let v1 = v0 + meshlet.vertex_count as usize;
            meshlet_vertices.extend_from_slice(&built.vertices[v0..v1]);

            let i0 = meshlet.triangle_offset as usize;
            let i1 = i0 + meshlet.triangle_count as usize * 3;
            meshlet_indices.extend_from_slice(&built.triangles[i0..i1]);

            let (parent_error, error) = parent_lod.unwrap_or((
                BoundingSphere::new(bounds.center.into(), f32::INFINITY),
                0.0,
            ));

            meshlets.push(Meshlet {
                vertex_offset,
                vertex_count: meshlet.vertex_count,
                index_offset,
                index_count: meshlet.triangle_count * 3,
                bounds,
                parent_error,
                error,
            });
        }
    }

    MeshletLod {
        vertices: meshlet_vertices,
        indices: meshlet_indices,
        meshlets,
    }
}
