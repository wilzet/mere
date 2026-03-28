use mere_mesh;
use std::{
    fs,
    path::{Path, PathBuf},
};

const ASSET_DIR: &str = "../assets";
const OUT_DIR: &str = "../assets/mere_processed";

fn main() -> anyhow::Result<()> {
    let out_dir = Path::new(OUT_DIR);
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
    let name = path.file_name().unwrap().to_str().unwrap();
    let output = out_dir.join(format!("{name}.mere"));
    let hash_path = out_dir.join(format!("{name}.hash"));

    let files = match collect_gltf_files(path) {
        Some(files) if !files.is_empty() => files,
        _ => {
            mere_log::warn!("{name} does not contain .gltf");
            return Ok(());
        }
    };

    let new_hash = hash_files(&files)?;
    if fs::read(&hash_path).is_ok_and(|old_hash| old_hash == new_hash) {
        mere_log::info!("Skipping {name}");
        return Ok(());
    }

    mere_log::info!("Processing {name}");

    let processed = process_meshes(&files)?;

    fs::write(&output, processed)?;
    fs::write(&hash_path, new_hash)?;

    Ok(())
}

fn collect_gltf_files(path: &Path) -> Option<Vec<PathBuf>> {
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

fn hash_files(paths: &[PathBuf]) -> anyhow::Result<Vec<u8>> {
    let mut hasher = blake3::Hasher::new();

    for path in paths {
        let data = fs::read(path)?;
        hasher.update(&data);
    }

    Ok(hasher.finalize().as_bytes().into())
}

pub fn process_meshes(paths: &[PathBuf]) -> anyhow::Result<Vec<u8>> {
    let mut merged_vertices = Vec::new();
    let mut total_indices = 0;

    for path in paths {
        let (gltf, buffers, _) = gltf::import(path)?;
        for mesh in gltf.meshes() {
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
        }
    }

    let mut mesh = mere_mesh::Mesh::new(merged_vertices, total_indices);
    mesh.optimize_mesh();

    mere_log::success!("Processed meshes");

    Ok(Vec::new())
}
