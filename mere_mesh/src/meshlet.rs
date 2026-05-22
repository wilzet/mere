use crate::VertexAttributes;
use itertools::Itertools;
use mere_math::Vec3;
use metis::{Graph, option::Opt};
use std::collections::HashMap;

#[repr(C)]
#[derive(Clone, Copy, Default, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Meshlet {
    pub vertex_offset: u32,
    pub attribute_offset: u32,
    pub vertex_count: u32,
    pub index_offset: u32,
    pub index_count: u32,
    // pub lod: u32,
    pub cull_data: CullData,
}

impl Meshlet {
    pub const MAX_VERTICES: usize = 255;
    pub const MAX_TRIANGLES: usize = 128;
    pub const MIN_TRIANGLES: usize = (Self::MAX_TRIANGLES / 3) & !3;
    pub const MAX_INDICES_PER_MESHLET: u32 = Self::MAX_TRIANGLES as u32 * 3;
    pub const FILL_WEIGHT: f32 = 1.0;
}

#[repr(C)]
#[derive(Clone, Copy, Default, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CullData {
    pub bounds: BoundingSphere,
    pub parent_error: BoundingSphere,
    pub error: f32,
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

// --- Meshlet Generation

pub fn generate_meshlets(
    vertices: &meshopt::VertexDataAdapter<'_>,
    indices: &[u32],
    vertex_positions_remap: &[u32],
    parent_lod: Option<(BoundingSphere, f32)>,
) -> (meshopt::Meshlets, Vec<CullData>) {
    if indices.is_empty() || indices.len() % 3 != 0 {
        return (
            meshopt::Meshlets {
                meshlets: Vec::new(),
                vertices: Vec::new(),
                triangles: Vec::new(),
            },
            Vec::new(),
        );
    }

    let triangle_count = indices.len() / 3;

    // Vertex -> triangle adjacency list tracking
    let mut vertices_to_triangles = vec![Vec::new(); vertex_positions_remap.len()];
    for (i, &index) in indices.iter().enumerate() {
        let vertex_id = vertex_positions_remap[index as usize];
        vertices_to_triangles[vertex_id as usize].push(i / 3);
    }

    // Calculate topological sharing across edges
    let mut pair_shared_count = HashMap::with_capacity(triangle_count * 2);
    for tri_ids in vertices_to_triangles {
        for (a, b) in tri_ids.into_iter().tuple_combinations() {
            let pair = (a.min(b), a.max(b));

            *pair_shared_count.entry(pair).or_insert(0) += 1;
        }
    }

    // Populate METIS Weighted Graph Structure
    let mut adjacency = vec![Vec::new(); triangle_count];
    for ((a, b), shared) in pair_shared_count {
        adjacency[a].push((b, shared));
        adjacency[b].push((a, shared));
    }

    for list in adjacency.iter_mut() {
        list.sort_unstable();
    }

    // Convert to Compressed Sparse Row (CSR) format
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

    let target_size = Meshlet::MAX_TRIANGLES - 4;
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

    let mut meshlets = meshopt::Meshlets {
        meshlets: Vec::new(),
        vertices: Vec::new(),
        triangles: Vec::new(),
    };
    let mut cull_data = Vec::new();
    let mut cluster_indices = Vec::new();

    for partition in partitions {
        cluster_indices.clear();
        cluster_indices.reserve(partition.len() * 3);

        for tri in partition {
            let i = tri * 3;
            cluster_indices.extend_from_slice(&indices[i..i + 3]);
        }

        let new_meshlets = meshopt::build_meshlets_spatial(
            &cluster_indices,
            vertices,
            Meshlet::MAX_VERTICES,
            Meshlet::MIN_TRIANGLES,
            Meshlet::MAX_TRIANGLES,
            Meshlet::FILL_WEIGHT,
        );

        cull_data.extend(new_meshlets.iter().map(|m| {
            let bounds = {
                let bounds = meshopt::compute_meshlet_bounds(m, vertices);
                BoundingSphere::new(bounds.center.into(), bounds.radius)
            };

            let (parent_error, error) = parent_lod
                .unwrap_or_else(|| (BoundingSphere::new(bounds.center.into(), f32::MAX), 0.0));

            CullData {
                bounds,
                parent_error,
                error,
            }
        }));

        merge_meshlets(&mut meshlets, new_meshlets);
    }

    (meshlets, cull_data)
}

pub fn merge_meshlets(meshlets: &mut meshopt::Meshlets, other: meshopt::Meshlets) {
    let vertex_offset = meshlets.vertices.len() as u32;
    let index_offset = meshlets.triangles.len() as u32;

    meshlets.vertices.extend_from_slice(&other.vertices);
    meshlets.triangles.extend_from_slice(&other.triangles);
    meshlets
        .meshlets
        .extend(other.meshlets.into_iter().map(|mut meshlet| {
            meshlet.vertex_offset += vertex_offset;
            meshlet.triangle_offset += index_offset;
            meshlet
        }));
}

pub fn build_per_meshlet_attributes(
    meshlet: &meshopt::ffi::meshopt_Meshlet,
    cull_data: CullData,
    meshlet_vertex_ids: &[u32],
    vertex_attributes: &[VertexAttributes],
    out_attributes: &mut Vec<VertexAttributes>,
    out_meshlets: &mut Vec<Meshlet>,
) {
    let attribute_offset = out_attributes.len() as u32;

    for &vertex_id in meshlet_vertex_ids {
        out_attributes.push(vertex_attributes[vertex_id as usize]);
    }

    out_meshlets.push(Meshlet {
        vertex_offset: meshlet.vertex_offset,
        attribute_offset,
        vertex_count: meshlet.vertex_count,
        index_offset: meshlet.triangle_offset,
        index_count: meshlet.triangle_count * 3,
        cull_data,
    });
}

// Code for meshlet generation based on:
//  * https://github.com/bevyengine/bevy/blob/d98861cc3da219a358953830f33d135b5342d013/crates/bevy_pbr/src/meshlet/from_mesh.rs
//  * https://github.com/zeux/meshoptimizer/blob/c619e7b941646e72ad1da67c058811207bcbcf88/demo/clusterlod.h
//
// Edited by permission of the MIT license:
//
// MIT License
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

