use std::{
    fs,
    path::{Path, PathBuf},
};

/// Recursively collects `.gltf` files under a path.
///
/// Returns [`None`] if no `.gltf` files are found.
///
/// # Panics
/// If reading a directory fails.
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
