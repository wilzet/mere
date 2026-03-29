use mere_asset::{ModelInstance, Scene, SceneObject};

fn main() {
    let mut scene = Scene::new();

    let teapot_model = match scene.add_model("utah_teapot") {
        Ok(handle) => handle,
        Err(err) => {
            mere_log::error!("{err}");
            return;
        }
    };

    let teapot = scene.add_object(SceneObject::model(
        teapot_model,
        mere_math::Transform::from_translation(mere_math::Vec3 {
            x: 1.0,
            y: -10.0,
            z: 2.0,
        }),
    ));

    let model_instance: ModelInstance = match scene.get_object(teapot).unwrap().try_into() {
        Ok(model) => model,
        Err(err) => {
            mere_log::error!("{err}");
            return;
        }
    };
    let model_from_handle = scene.get_model(model_instance.handle()).unwrap();
    mere_log::info!("{teapot:?}");
    mere_log::info!("{model_instance:?}");
    for mesh in model_from_handle.meshes() {
        mere_log::info!(
            "vertices: {} indices: {} triangles: {}",
            mesh.vertices.len(),
            mesh.indices.len(),
            mesh.indices.len() / 3
        );
    }
}
