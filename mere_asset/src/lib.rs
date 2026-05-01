//! Asset types and scene representation.
//!
//! Provides core data structures used at runtime:
//! - Models and meshes
//! - Materials and textures
//! - Scene and objects
//! - Camera

mod asset;
mod asset_server;
mod camera;
mod gpu_buffer;
mod handle;
mod instances;
mod material;
mod meshlets;
mod resources;
mod scene;
mod texture;

pub use {
    camera::Camera,
    material::Material,
    resources::{MeshletBindGroups, ResourceStorage},
    scene::Scene,
    texture::Texture,
};
