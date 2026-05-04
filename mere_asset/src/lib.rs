//! Asset types and scene representation.
//!
//! Provides core data structures used at runtime:
//! - Models and meshes
//! - Materials and textures
//! - World and objects
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
mod texture;
mod world;

pub use {
    camera::Camera,
    instance_storage::{Instance, InstanceStorage},
    material::Material,
    resource_storage::ResourceStorage,
    texture::Texture,
    world::World,
};
