use crate::{
    Material, Mesh, Model,
    asset_server::AssetServer,
    handle::{ResourceHandle, UntypedHandle},
    texture::{Texture, TextureOptions},
};
use common::collect_gltf_files;
use gltf::{Material as GltfMaterial, Mesh as GltfMesh};
use image::GenericImageView;
use mere_common::{ASSET_DIR, PROCESSED_ASSET_DIR};
use mere_mesh::{MereMesh, read_mere_file};
use std::path;

pub(crate) trait Asset: Sized {
    type Source<'a>;

    fn load(source: Self::Source<'_>) -> anyhow::Result<Self>;
    fn dependencies(&self) -> Vec<UntypedHandle>;
    fn finish(&mut self, _asset_server: &AssetServer) {}
}

impl Asset for Model {
    type Source<'a> = (&'a str, GltfMesh<'a>, Vec<MereMesh>, &'a wgpu::Device);

    fn load(source: Self::Source<'_>) -> anyhow::Result<Self> {
        let (path, model, meshes, device) = source;
        let default_name = format!("{path}_model_{}", model.index());
        let name = model.name().unwrap_or(&default_name);

        let meshes = model
            .primitives()
            .zip(meshes)
            .map(|(primitive, mere_mesh)| {
                let material = match primitive.material().name() {
                    Some(name) => ResourceHandle::from(name),
                    None => Material::DEFAULT_MATERIAL_ID,
                };

                Mesh::from_mere_mesh(name, mere_mesh, &device).with_material(material)
            })
            .collect();

        Ok(Self::new(name, meshes))
    }

    fn dependencies(&self) -> Vec<UntypedHandle> {
        vec![]
    }
}

impl Asset for Texture {
    type Source<'a> = (
        &'a path::Path,
        &'a wgpu::Device,
        &'a wgpu::Queue,
        TextureOptions,
        u32,
    );

    fn load(source: Self::Source<'_>) -> anyhow::Result<Self> {
        let (path, device, queue, options, mip_0) = source;

        let label = match path.file_name().map_or(None, |s| s.to_str()) {
            Some(name) => name,
            None => anyhow::bail!("No file found in path: {path:?}"),
        };

        let image = load_image(path, mip_0)?;
        Ok(Self::from_image(device, queue, image, label, options)?)
    }

    fn dependencies(&self) -> Vec<UntypedHandle> {
        vec![]
    }
}

impl Asset for Material {
    type Source<'a> = (&'a str, GltfMaterial<'a>, &'a wgpu::Device, &'a AssetServer);

    fn load(source: Self::Source<'_>) -> anyhow::Result<Self> {
        let (path, material, device, asset_server) = source;

        let default_name = format!("{path}_mat_{}", material.index().unwrap_or(0));
        let name = material.name().unwrap_or(&default_name);

        Ok(Self::from_gltf_material(
            name,
            material,
            device,
            asset_server,
        ))
    }

    fn dependencies(&self) -> Vec<UntypedHandle> {
        vec![
            self.diffuse.into(),
            self.rough_metal.into(),
            self.normal.into(),
        ]
    }

    fn finish(&mut self, asset_server: &AssetServer) {
        self.try_finish(&asset_server.device, asset_server);
    }
}

pub(crate) struct MereAsset {
    meshes: Vec<MereMesh>,
}

impl MereAsset {
    pub fn meshes(self) -> impl Iterator<Item = MereMesh> {
        self.meshes.into_iter()
    }
}

pub(crate) fn load_mere_asset(path: impl AsRef<path::Path>) -> anyhow::Result<MereAsset> {
    let model_path = path::PathBuf::from(PROCESSED_ASSET_DIR)
        .join(&path)
        .with_extension("mere");

    let meshes = read_mere_file(&model_path)?;

    Ok(MereAsset { meshes })
}

fn load_image(uri: &path::Path, mip: u32) -> anyhow::Result<image::DynamicImage> {
    let image = image::open(uri)?;
    let (width, height) = image.dimensions();
    let factor = 1 << mip;
    Ok(image.resize(
        width / factor,
        height / factor,
        image::imageops::FilterType::Triangle,
    ))
}

pub(crate) struct GltfAsset {
    document: gltf::Document,
}

impl GltfAsset {
    pub fn document(&self) -> &gltf::Document {
        &self.document
    }

    pub fn images(&self) -> Vec<gltf::Image<'_>> {
        self.document.images().collect()
    }

    pub fn materials(&self) -> Vec<gltf::Material<'_>> {
        self.document.materials().collect()
    }
}

pub(crate) fn load_gltf_asset(path: impl AsRef<path::Path>) -> anyhow::Result<GltfAsset> {
    let gltf_path = path::PathBuf::from(ASSET_DIR).join(&path);
    let gltf_paths = collect_gltf_files(&gltf_path).unwrap();
    assert!(gltf_paths.len() == 1);
    let gltf_path = &gltf_paths[0];

    Ok(GltfAsset {
        document: gltf::Gltf::open(gltf_path)?.document,
    })
}
