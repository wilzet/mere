//! Asset build script.
//!
//! Converts source assets (GLTF) into `.mere` files.
//! Skips unchanged assets using content hashing.
//!
//! Runs in release or when `BUILD_ASSETS=1` is set.

use anyhow::Context;
use mere_asset_common::collect_gltf_files;
use mere_common::{ASSET_DIR, PROCESSED_ASSET_DIR};
use mere_mesh::{Mesh, write_mere_file};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::{
    fs, io,
    path::{Path, PathBuf},
    time::Instant,
};

fn main() -> anyhow::Result<()> {
    println!("cargo:rerun-if-env-changed=BUILD_ASSETS");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={ASSET_DIR}");

    let asset_processing_enabled = std::env::var("BUILD_ASSETS").is_ok();
    let profile = std::env::var("PROFILE").unwrap_or("debug".to_string());
    if profile != "release" && !asset_processing_enabled {
        mere_log::info!("Skipping assets processing (use `BUILD_ASSETS=1` to enable)");
        return Ok(());
    }

    let out_dir = Path::new(PROCESSED_ASSET_DIR);
    if let Err(err) = fs::create_dir_all(out_dir) {
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
            mere_log::info!("{asset_root_name} does not contain .gltf");
            return Ok(());
        }
    };

    let hash_path = out_dir.join(asset_root_name).with_extension("hash");
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

        let processed_meshes = process_meshes(&file)?;
        write_mere_file(&output_path, processed_meshes)?;
    }

    fs::write(&hash_path, new_hash)?;

    Ok(())
}

fn hash_files(paths: &[PathBuf]) -> anyhow::Result<Vec<u8>> {
    let mut hasher = blake3::Hasher::new();

    for path in paths {
        let mut file = fs::File::open(path)?;

        io::copy(&mut file, &mut hasher)?;
    }

    Ok(hasher.finalize().as_bytes().into())
}

fn process_meshes(path: &PathBuf) -> anyhow::Result<Vec<mere_mesh::Mesh>> {
    let start = Instant::now();

    let (gltf, buffers, _) = gltf::import(path)?;
    let meshes = gltf
        .meshes()
        .collect::<Vec<_>>()
        .into_par_iter()
        .flat_map(|model| {
            model
                .primitives()
                .collect::<Vec<_>>()
                .into_par_iter()
                .map(|p| {
                    let mut mesh = Mesh::from_gltf_primitive(p, &buffers);
                    mesh.optimize_mesh();
                    mesh
                })
        })
        .collect::<Vec<_>>();

    let time = start.elapsed();

    mere_log::success!(
        "Processed {} mesh(es) from {:?} ({:.3} ms)",
        meshes.len(),
        path.file_name().unwrap(),
        time.as_secs_f32() * 1000.0,
    );

    Ok(meshes)
}
