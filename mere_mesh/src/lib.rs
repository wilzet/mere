//! Custom mesh format and utilities for reading/writing engine mesh data.

mod mesh;
mod meshlet;
mod util;
mod vertex;

pub use {
    mesh::{Aabb, MeshletMesh, read_mere_file, write_mere_file},
    meshlet::Meshlet,
    vertex::{Vertex, VertexAttributes},
};
