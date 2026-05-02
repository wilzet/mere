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
mod instance_storage;
mod material;
mod meshlet_storage;
mod resource_storage;
mod scene;
mod texture;

pub use {
    camera::Camera,
    material::Material,
    resource_storage::{MeshletBindGroups, ResourceStorage},
    scene::Scene,
    texture::Texture,
};
