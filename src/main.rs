use mere_asset::{MeshInstance, Scene, SceneObject};

fn main() {
    let mut scene = Scene::new();

    let teapot_mesh = match scene.add_mesh("utah_teapot") {
        Ok(mesh_handle) => mesh_handle,
        Err(err) => {
            mere_log::error!("{err}");
            return;
        }
    };

    let teapot = scene.add_object(SceneObject::mesh(
        teapot_mesh,
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
