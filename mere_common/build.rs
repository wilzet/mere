fn main() {
    let cwd = std::env::current_dir().unwrap();

    // Logic to find the absolute path to "<project>/assets"
    let (asset_path, mere_path) = if cwd.ends_with("project") {
        let asset_path = cwd.join("assets");
        let mere_path = asset_path.join("mere_processed");
        (asset_path, mere_path)
    } else {
        let parent = cwd.parent().unwrap();
        let asset_path = parent.join("assets");
        let mere_path = asset_path.join("mere_processed");
        (asset_path, mere_path)
    };

    println!(
        "cargo:rustc-env=MERE_ASSETS_ROOT_DIR={}",
        asset_path.display()
    );
    println!(
        "cargo:rustc-env=MERE_ASSETS_PROCESSED_DIR={}",
        mere_path.display()
    );
}
