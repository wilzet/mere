use anyhow::Context;
use common::{collect_gltf_files, pack_10_10_10_2, pack_11_11_10, pack_16_16, write_mere_file};
use mere_common::{ASSET_DIR, PROCESSED_ASSET_DIR};
use mere_math::{Vec2, Vec3};
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
        let mut file = fs::File::open(path)?;

        io::copy(&mut file, &mut hasher)?;
    }

    Ok(hasher.finalize().as_bytes().into())
}

pub fn process_meshes(path: &PathBuf) -> anyhow::Result<Vec<mere_mesh::MereMesh>> {
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
                .map(|primitive| {
                    let reader = primitive.reader(|b| Some(&buffers[b.index()]));

                    let indices_iter = reader.read_indices().unwrap().into_u32();
                    let index_count = indices_iter.len();

                    let (positions, normals, tex_coords, tangents) = match reader.read_tangents() {
                        Some(tangents) => {
                            let positions = reader
                                .read_positions()
                                .unwrap()
                                .map(Vec3::from)
                                .collect::<Vec<_>>();
                            let normals = reader.read_normals().map_or_else(
                                || vec![0; positions.len()],
                                |it| it.map(|n| pack_11_11_10(Vec3::from(n))).collect(),
                            );
                            let tex_coords = reader.read_tex_coords(0).map_or_else(
                                || vec![0; positions.len()],
                                |it| it.into_f32().map(|t| pack_16_16(Vec2::from(t))).collect(),
                            );
                            let tangents = tangents
                                .map(|t| {
                                    pack_10_10_10_2(
                                        Vec3::new(t[0], t[1], t[2]),
                                        ((t[3].signum() as i32 + 1) >> 1) as u32,
                                    )
                                })
                                .collect::<Vec<_>>();

                            (positions, normals, tex_coords, tangents)
                        }
                        None => {
                            let positions = reader
                                .read_positions()
                                .unwrap()
                                .map(Vec3::from)
                                .collect::<Vec<_>>();
                            let normals = reader.read_normals().map_or_else(
                                || vec![Vec3::ZERO; positions.len()],
                                |it| it.map(Vec3::from).collect(),
                            );
                            let tex_coords = reader.read_tex_coords(0).map_or_else(
                                || vec![Vec2::ZERO; positions.len()],
                                |it| it.into_f32().map(Vec2::from).collect(),
                            );
                            let tangents = reader.read_tangents().map_or_else(
                                || {
                                    calculate_tangents(
                                        &indices_iter.clone().collect::<Vec<_>>(),
                                        &positions,
                                        &normals,
                                        &tex_coords,
                                    )
                                },
                                |it| {
                                    it.map(|t| {
                                        pack_10_10_10_2(
                                            Vec3::new(t[0], t[1], t[2]),
                                            ((t[3].signum() as i32 + 1) >> 1) as u32,
                                        )
                                    })
                                    .collect()
                                },
                            );

                            let normals =
                                normals.into_iter().map(pack_11_11_10).collect::<Vec<_>>();
                            let tex_coords =
                                tex_coords.into_iter().map(pack_16_16).collect::<Vec<_>>();

                            (positions, normals, tex_coords, tangents)
                        }
                    };

                    let vertices = indices_iter.map(|i| {
                        let index = i as usize;
                        mere_mesh::Vertex {
                            position: positions[index],
                            normal: normals[index],
                            tex_coord: tex_coords[index],
                            tangent: tangents[index],
                        }
                    });

                    let mut mesh = mere_mesh::MereMesh::new(vertices, index_count);
                    mesh.optimize_mesh();
                    mesh
                })
        })
        .collect::<Vec<_>>();

    let time = start.elapsed();

    mere_log::success!(
        "Processed {} mesh(es) in {:?} ({:.3} ms)",
        meshes.len(),
        path.file_name().unwrap(),
        time.as_secs_f32() * 1000.0,
    );

    Ok(meshes)
}

fn calculate_tangents(
    indices_iter: &[u32],
    positions: &[Vec3],
    normals: &[Vec3],
    tex_coords: &[Vec2],
) -> Vec<u32> {
    let mut tangents = vec![Vec3::ZERO; positions.len()];
    let mut bitangents = vec![Vec3::ZERO; positions.len()];

    for chunk in indices_iter.chunks(3) {
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
                return pack_10_10_10_2(Vec3::X, 1);
            }

            let b = bitangents[i];
            let n = normals[i];

            let t_ortho = (t - n * n.dot(t)).normalize();

            let handedness = if n.cross(t_ortho).dot(b) < 0.0 {
                // negative
                0
            } else {
                1
            };

            pack_10_10_10_2(t_ortho, handedness)
        })
        .collect()
}
