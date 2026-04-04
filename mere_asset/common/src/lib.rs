use mere_mesh::MereMesh;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn collect_gltf_files(path: &Path) -> Option<Vec<PathBuf>> {
    if path.is_dir() {
        let mut files = Vec::new();
        for entry in fs::read_dir(path).unwrap().filter_map(Result::ok) {
            let path = entry.path();
            if let Some(gltf_files) = collect_gltf_files(&path) {
                files.extend(gltf_files);
            }
        }

        Some(files)
    } else if path.extension().is_some_and(|ext| ext == "gltf") {
        Some(vec![path.into()])
    } else {
        None
    }
}

pub fn write_mere_file(output_path: &Path, meshes: Vec<MereMesh>) -> anyhow::Result<()> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mesh_count = meshes.len();
    let mut bin = (mesh_count as u32).to_le_bytes().to_vec();
    bin.extend_from_slice(
        meshes
            .iter()
            .flat_map(|m| m.into_mere_file())
            .collect::<Vec<_>>()
            .as_slice(),
    );

    fs::write(&output_path, bin)?;

    Ok(())
}

pub fn read_mere_file(path: &Path) -> anyhow::Result<Vec<MereMesh>> {
    let mere_bytes = fs::read(&path)?;
    let mesh_count = u32::from_le_bytes(mere_bytes[0..4].try_into()?) as usize;

    let mut offset = 4;
    let mut meshes = Vec::new();
    for _ in 0..mesh_count {
        let (mesh, read_bytes) = MereMesh::from_mere_file(&mere_bytes[offset..])?;
        meshes.push(mesh);
        offset += read_bytes;
    }

    Ok(meshes)
}
