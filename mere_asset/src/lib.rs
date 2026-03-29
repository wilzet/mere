mod asset;
mod camera;
mod mesh;
mod scene;

pub use camera::Camera;
pub use mesh::{MeshHandle, MeshInstance};
pub use scene::{Scene, SceneObject};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scene() {
        let mut scene = Scene::new();

        let handle = scene.add_mesh("utah_teapot").unwrap();
        let teapot = scene.add_object(SceneObject::mesh(
            handle,
            mere_math::Transform::from_translation(mere_math::Vec3 {
                x: 1.0,
                y: -10.0,
                z: 2.0,
            }),
        ));

        let mesh_instance: MeshInstance = match scene.get_object(teapot).unwrap().try_into() {
            Ok(mesh_instance) => mesh_instance,
            Err(err) => {
                mere_log::error!("{err}");
                return;
            }
        };
        let mesh_from_handle = scene.get_mesh(mesh_instance.handle()).unwrap();
        mere_log::info!("{teapot:?}");
        mere_log::info!("{mesh_instance:?}");
        mere_log::info!(
            "vertices: {} indices: {} triangles: {}",
            mesh_from_handle.vertices.len(),
            mesh_from_handle.indices.len(),
            mesh_from_handle.indices.len() / 3
        );
    }
}
