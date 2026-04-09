use anyhow::Context;
use common::{collect_gltf_files, write_mere_file};
use mere_common::{ASSET_DIR, PROCESSED_ASSET_DIR};
use mere_math::{Vec2, Vec3, Vec4};
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
        let data = fs::read(path)?;
        hasher.update(&data);
    }

    Ok(hasher.finalize().as_bytes().into())
}

pub fn process_meshes(path: &PathBuf) -> anyhow::Result<Vec<mere_mesh::MereMesh>> {
    let mut meshes = Vec::new();

    let (gltf, buffers, _) = gltf::import(path)?;
    for model in gltf.meshes() {
        for primitive in model.primitives() {
            let reader = primitive.reader(|b| Some(&buffers[b.index()]));

            let indices = reader.read_indices().unwrap().into_u32();
            let index_count = indices.len();

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
            let tangents = reader
                .read_tangents()
                .map(|it| it.map(|t| Vec4::from(t)).collect())
                .unwrap_or_else(|| {
                    calculate_tangents(indices.clone().collect(), &positions, &normals, &tex_coords)
                });

            let vertices = indices.map(|i| {
                let index = i as usize;
                mere_mesh::Vertex {
                    position: positions[index],
                    normal: normals[index],
                    tex_coord: tex_coords[index],
                    tangent: tangents[index].into(),
                }
            });

            let mut mesh = mere_mesh::MereMesh::new(vertices, index_count);
            mesh.optimize_mesh();
            meshes.push(mesh);
        }
    }

    mere_log::success!("Processed meshes in {path:?}");

    Ok(meshes)
}

fn calculate_tangents(
    indices: Vec<u32>,
    positions: &[Vec3],
    normals: &[Vec3],
    tex_coords: &[Vec2],
) -> Vec<Vec4> {
    let mut tangents = vec![Vec3::ZERO; positions.len()];
    let mut bitangents = vec![Vec3::ZERO; positions.len()];

    for chunk in indices.chunks(3) {
        let i0 = chunk[0] as usize;
        let i1 = chunk[1] as usize;
        let i2 = chunk[2] as usize;

        let edge0 = positions[i1] - positions[i0];
        let edge1 = positions[i2] - positions[i0];
        let delta_uv0 = tex_coords[i1] - tex_coords[i0];
        let delta_uv1 = tex_coords[i2] - tex_coords[i0];

        let det = delta_uv0.x * delta_uv1.y - delta_uv1.x * delta_uv0.y;

        let (tangent, bitangent) = if det.abs() < f32::EPSILON {
            let normal = normals[i0];
            let helper = if normal.x.abs() < 0.9 {
                Vec3::X
            } else {
                Vec3::Y
            };
            let t = normal.cross(helper);
            let b = normal.cross(t);
            (t, b)
        } else {
            let f = 1.0 / det;
            (
                (edge0 * delta_uv1.y - edge1 * delta_uv0.y) * f,
                (edge1 * delta_uv0.x - edge0 * delta_uv1.x) * f,
            )
        };

        tangents[i0] += tangent;
        tangents[i1] += tangent;
        tangents[i2] += tangent;

        bitangents[i0] += bitangent;
        bitangents[i1] += bitangent;
        bitangents[i2] += bitangent;
    }

    tangents
        .into_iter()
        .enumerate()
        .map(|(i, t)| {
            if t.length_squared() == 0.0 {
                return Vec4::new(1.0, 0.0, 0.0, 1.0);
            }

            let b = bitangents[i];
            let n = normals[i];

            let t_ortho = (t - n * n.dot(t)).normalize();

            let handedness = if n.cross(t_ortho).dot(b) < 0.0 {
                -1.0
            } else {
                1.0
            };

            t_ortho.extend(handedness)
        })
        .collect()
}
