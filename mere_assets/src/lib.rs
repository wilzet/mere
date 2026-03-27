use std::{error::Error as StdError, fs, io};

fn load_gltf(path: &str) -> Result<gltf::Gltf, Box<dyn StdError>> {
    let file = fs::File::open(path)?;
    let reader = io::BufReader::new(file);
    let gltf = gltf::Gltf::from_reader(reader)?;
    Ok(gltf)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find_gltf_files(dir: &std::path::Path) {
        for entry in fs::read_dir(dir).unwrap().filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                find_gltf_files(&path);
            } else if path.extension().is_some_and(|ext| ext == "gltf") {
                if let Ok(gltf) =
                    load_gltf(path.to_str().expect("path should have valid UTF-8 name"))
                {
                    println!(
                        "asset: {:#?}\n\tused extension: {:?}\n\trequired extensions: {:?}",
                        gltf.nodes().map(|n| n.name()).collect::<Vec<_>>(),
                        gltf.extensions_used(),
                        gltf.extensions_required()
                    );
                }
            }
        }
    }

    #[test]
    fn it_works() {
        let path = std::env::current_dir()
            .expect("should get cwd")
            .join("../assets");

        find_gltf_files(&path);
    }
}
