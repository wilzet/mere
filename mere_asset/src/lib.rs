mod asset;
mod camera;
mod handle;
mod material;
mod model;
mod scene;
mod texture;

pub use camera::Camera;
pub use scene::{Scene, SceneObject, SceneObjectHandle};
pub use texture::Texture;

#[cfg(test)]
mod tests {
    use crate::asset::load_mere_asset;

    #[test]
    fn test_scene() {
        let teapot_mere = match load_mere_asset("sponza/pkg_a_curtains") {
            Ok(mere) => mere,
            Err(err) => {
                mere_log::error!("{err}");
                panic!();
            }
        };

        let meshes = teapot_mere.meshes().collect::<Vec<_>>();
        mere_log::info!("meshes: {}", meshes.len());
        for mesh in meshes {
            mere_log::info!(
                "vertices: {} indices: {} triangles: {}",
                mesh.vertices.len(),
                mesh.indices.len(),
                mesh.indices.len() / 3
            );
        }
    }
}
