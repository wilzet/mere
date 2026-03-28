use common::collect_gltf_files;
use mere::{ASSET_DIR, PROCESSED_ASSET_DIR};
use std::{fs, io, path};

mod camera;
mod mesh;
mod scene;

pub use camera::Camera;
pub use mesh::{MeshHandle, MeshInstance};
pub use scene::{Scene, SceneObject};

pub struct MeshAsset {
    pub gltf: gltf::Document,
    pub mesh: mere_mesh::Mesh,
}

pub fn load_mere_asset(path: impl AsRef<path::Path>) -> anyhow::Result<MeshAsset> {
    let mesh_path = path::PathBuf::from(PROCESSED_ASSET_DIR)
        .join(&path)
        .with_extension("mere");

    let gltf_path = path::PathBuf::from(ASSET_DIR).join(&path);
    let gltf_paths = collect_gltf_files(&gltf_path).unwrap();
    assert!(gltf_paths.len() == 1);
    let gltf_path = &gltf_paths[0];

    let mere_bytes = fs::read(&mesh_path)?;
    let mesh = mere_mesh::Mesh::from_mere_file(&mere_bytes[..])?;

    let file = fs::File::open(&gltf_path)?;
    let reader = io::BufReader::new(file);
    let json = gltf::json::deserialize::from_reader(reader)?;
    let document = gltf::Document::from_json(json)?;

    Ok(MeshAsset {
        gltf: document,
        mesh,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scene() {
        let mut scene = Scene::new();

        let mesh = load_mere_asset("sponza/pkg_a_curtains").unwrap();

        let handle = scene.add_mesh(mesh.mesh);
        let teapot = scene.add_object(SceneObject::mesh(
            handle,
            mere_math::Transform::from_translation(mere_math::Vec3 {
                x: 1.0,
                y: -10.0,
                z: 2.0,
            }),
        ));

        mere_log::info!("{teapot:?}");
    }
}
