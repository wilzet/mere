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
mod handle;
mod material;
mod model;
mod scene;
mod texture;

pub use {
    camera::Camera,
    material::Material,
    model::{Mesh, Model},
    scene::{Scene, SceneObject, SceneObjectHandle},
    texture::Texture,
};
