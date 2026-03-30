use anyhow::Context;
use common::collect_gltf_files;
use mere_common::{ASSET_DIR, PROCESSED_ASSET_DIR};
use mere_math::{Vec2, Vec3};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn main() -> anyhow::Result<()> {
    println!("cargo:rerun-if-env-changed=BUILD_ASSETS");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={ASSET_DIR}");

    let asset_processing_enabled = std::env::var("BUILD_ASSETS").is_ok();
    let profile = std::env::var("PROFILE").unwrap_or("debug".to_string());
    if profile != "release" && !asset_processing_enabled {
        mere_log::info!("Skipping assets processing (use `BUILD_ASSETS=1 cargo run` to enable)");
        return Ok(());
    }

    let out_dir = Path::new(PROCESSED_ASSET_DIR);
    if let Err(err) = fs::create_dir_all(&out_dir) {
        mere_log::error!(return err);
    }

    for entry in fs::read_dir(ASSET_DIR)?.filter_map(Result::ok) {
        let path = entry.path();
        if let Err(err) = process_asset(&path, out_dir) {
            mere_log::error!(return err);
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

            let positions = reader
                .read_positions()
                .unwrap()
                .map(|p| Vec3::from(p))
                .collect::<Vec<_>>();
            let normals = reader
                .read_normals()
                .map(|it| it.map(|n| Vec3::from(n)).collect())
                .unwrap_or_else(|| vec![Vec3::ZERO; positions.len()]);
            let tex_coords = reader
                .read_tex_coords(0)
                .map(|it| it.into_f32().map(|t| Vec2::from(t)).collect())
                .unwrap_or_else(|| vec![Vec2::ZERO; positions.len()]);
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
