use common::{collect_gltf_files, read_mere_file};
use mere_common::{ASSET_DIR, PROCESSED_ASSET_DIR};
use mere_mesh::MereMesh;
use std::path;

pub(crate) struct MereAsset {
    meshes: Vec<MereMesh>,
}

impl MereAsset {
    pub fn meshes(self) -> impl Iterator<Item = MereMesh> {
        self.meshes.into_iter()
    }
}

pub(crate) fn load_mere_asset(path: impl AsRef<path::Path>) -> anyhow::Result<MereAsset> {
    let model_path = path::PathBuf::from(PROCESSED_ASSET_DIR)
        .join(&path)
        .with_extension("mere");

    let meshes = read_mere_file(&model_path)?;

    Ok(MereAsset { meshes })
}

pub(crate) struct GltfAsset {
    document: gltf::Document,
    images: Vec<gltf::image::Data>,
}

impl GltfAsset {
    pub fn document(&self) -> &gltf::Document {
        &self.document
    }

    pub fn images(&self) -> &Vec<gltf::image::Data> {
        &self.images
    }
}

pub(crate) fn load_gltf_asset(path: impl AsRef<path::Path>) -> anyhow::Result<GltfAsset> {
    let gltf_path = path::PathBuf::from(ASSET_DIR).join(&path);
    let gltf_paths = collect_gltf_files(&gltf_path).unwrap();
    assert!(gltf_paths.len() == 1);
    let gltf_path = &gltf_paths[0];

    let (document, _, images) = gltf::import(gltf_path)?;
    Ok(GltfAsset { document, images })
}
