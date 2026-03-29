use anyhow::Context;
use common::collect_gltf_files;
use mere_common::{ASSET_DIR, PROCESSED_ASSET_DIR};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn main() -> anyhow::Result<()> {
    let out_dir = Path::new(PROCESSED_ASSET_DIR);
    match fs::create_dir_all(&out_dir) {
        Ok(_) => (),
        Err(err) => {
            mere_log::error!("{err}");
            return Err(err.into());
        }
    }

    for entry in fs::read_dir(ASSET_DIR)?.filter_map(Result::ok) {
        let path = entry.path();
        match process_asset(&path, out_dir) {
            Ok(_) => (),
            Err(err) => {
                mere_log::error!("{err}");
                return Err(err.into());
            }
        }
    }

    Ok(())
}

fn process_asset(path: &Path, out_dir: &Path) -> anyhow::Result<()> {
    let asset_root_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .context(format!("Invalid path {path:?}"))?;

    let files = match collect_gltf_files(path) {
        Some(files) if !files.is_empty() => files,
        _ => {
            mere_log::warn!("{asset_root_name} does not contain .gltf");
            return Ok(());
        }
    };

    let hash_path = out_dir.join(format!("{asset_root_name}.hash"));
    let new_hash = hash_files(&files)?;
    if fs::read(&hash_path).is_ok_and(|old_hash| old_hash == new_hash) {
        mere_log::info!("Skipping {asset_root_name}");
        return Ok(());
    }

    let file_count = files.len();
    for file in files {
        let asset_name = file
            .parent()
            .and_then(|p| p.file_stem())
            .and_then(|s| s.to_str())
            .context(format!("Invalid path {file:?}"))?;

        let output_path = out_dir
            .join(asset_root_name)
            .join(if file_count > 1 { asset_name } else { "" })
            .with_extension("mere");

        mere_log::info!("Processing {asset_root_name} -> {asset_name}");

        let processed = process_meshes(&file)?;
        let bin = processed.into_mere_file()?;

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&output_path, bin)?;
    }

    fs::write(&hash_path, new_hash)?;

    Ok(())
}

fn hash_files(paths: &[PathBuf]) -> anyhow::Result<Vec<u8>> {
    let mut hasher = blake3::Hasher::new();

    for path in paths {
        let data = fs::read(path)?;
        hasher.update(&data);
    }

    Ok(hasher.finalize().as_bytes().into())
}

pub fn process_meshes(path: &PathBuf) -> anyhow::Result<mere_mesh::Model> {
    let mut meshes = Vec::new();

    let (gltf, buffers, _) = gltf::import(path)?;
    for mesh in gltf.meshes() {
        let mut merged_vertices = Vec::new();
        let mut total_indices = 0;
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|b| Some(&buffers[b.index()]));

            let indices = reader.read_indices().unwrap().into_u32();
            total_indices += indices.len();

            let positions: Vec<_> = reader.read_positions().unwrap().collect();
            let normals: Vec<_> = reader.read_normals().map(|it| it.collect()).unwrap_or(
                [[0f32, 0f32, 0f32]]
                    .into_iter()
                    .cycle()
                    .take(positions.len())
                    .collect(),
            );
            let tex_coords: Vec<_> = reader
                .read_tex_coords(0)
                .map(|it| it.into_f32().collect())
                .unwrap_or(
                    [[0f32, 0f32]]
                        .into_iter()
                        .cycle()
                        .take(positions.len())
                        .collect(),
                );
            let vertices = indices.map(|i| {
                let index = i as usize;
                let position = positions[index];
                let normal = normals[index];
                let tex_coord = tex_coords[index];
                mere_mesh::Vertex {
                    position,
                    normal,
                    tex_coord,
                }
            });

            merged_vertices.extend(vertices);
        }

        let mut mesh = mere_mesh::Mesh::new(merged_vertices, total_indices);
        mesh.optimize_mesh();

        meshes.push(mesh);
    }

    mere_log::success!("Processed meshes in {path:?}");

    Ok(mere_mesh::Model::from(meshes))
}
