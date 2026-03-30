fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    // Use CARGO_MANIFEST_DIR to get a stable, absolute path to this crate
    let manifest_dir = std::path::PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").unwrap()
    );

    // Assuming structure is: 
    // project/
    // ├── assets/
    // ├───── mere_processed/
    // :
    // ├── mere_common/ (where this build.rs lives)
    // :
    let asset_path = manifest_dir.parent().unwrap().join("assets");
    let mere_path = asset_path.join("mere_processed");

    println!(
        "cargo:rustc-env=MERE_ASSETS_ROOT_DIR={}",
        asset_path.display()
    );
    println!(
        "cargo:rustc-env=MERE_ASSETS_PROCESSED_DIR={}",
        mere_path.display()
    );
}
