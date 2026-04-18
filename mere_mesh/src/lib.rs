//! Custom mesh format and utilities for reading/writing engine mesh data.

mod mesh;
mod util;
mod vertex;

pub use mesh::{MereMesh, read_mere_file, write_mere_file};
pub use vertex::Vertex;
