use common::collect_gltf_files;
use mere::{ASSET_DIR, PROCESSED_ASSET_DIR};
use std::{fs, io, path};

pub struct MereAsset {
    mesh: mere_mesh::Mesh,
}

impl MereAsset {
    pub fn mesh(self) -> mere_mesh::Mesh {
        self.mesh
    }
}

pub(crate) fn load_mere_asset(path: impl AsRef<path::Path>) -> anyhow::Result<MereAsset> {
    let mesh_path = path::PathBuf::from(PROCESSED_ASSET_DIR)
        .join(&path)
        .with_extension("mere");

    let mere_bytes = fs::read(&mesh_path)?;
    let mesh = mere_mesh::Mesh::from_mere_file(&mere_bytes[..])?;

    Ok(MereAsset { mesh })
}

pub struct GltfAsset {
    document: gltf::Document,
}

pub(crate) fn load_gltf_asset(path: impl AsRef<path::Path>) -> anyhow::Result<GltfAsset> {
    let gltf_path = path::PathBuf::from(ASSET_DIR).join(&path);
    let gltf_paths = collect_gltf_files(&gltf_path).unwrap();
    assert!(gltf_paths.len() == 1);
    let gltf_path = &gltf_paths[0];

    let file = fs::File::open(&gltf_path)?;
    let reader = io::BufReader::new(file);
    let json = gltf::json::deserialize::from_reader(reader)?;
    let document = gltf::Document::from_json(json)?;

    Ok(GltfAsset { document })
}
