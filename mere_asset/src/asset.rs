use crate::{handle::ResourceHandle, model::Model, texture::Texture};
use common::{collect_gltf_files, read_mere_file};
use image::GenericImageView;
use mere_common::{ASSET_DIR, PROCESSED_ASSET_DIR};
use mere_mesh::MereMesh;
use std::{collections::HashMap, path};

#[derive(Clone, Debug)]
pub(crate) struct AssetServer {
    models: HashMap<ResourceHandle<Model>, Model>,
    textures: HashMap<ResourceHandle<Texture>, Texture>,
}

impl AssetServer {
    const DEFAULT_IMAGE_LOAD_MIP: u32 = 5;

    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let mut asset_server = Self {
            models: HashMap::new(),
            textures: HashMap::new(),
        };

        Texture::DEFAULT_TEXTURES
            .iter()
            .for_each(|&(id, name, color)| {
                let label = Some(name);
                let texture = match Texture::from_bytes(device, queue, 1, 1, &color, label) {
                    Ok(tex) => tex,
                    Err(err) => {
                        mere_log::error!("{err}");
                        return;
                    }
                };

                asset_server.textures.insert(id.into(), texture);
            });

        asset_server
    }

    pub fn add_model(&mut self, model: Model) -> anyhow::Result<ResourceHandle<Model>> {
        let id = ResourceHandle::from(model.name());
        if let Some(_) = self.models.get(&id) {
            return Ok(id);
        }

        self.models.insert(id, model);
        Ok(id)
    }

    pub fn add_texture(
        &mut self,
        path: &path::PathBuf,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> anyhow::Result<ResourceHandle<Texture>> {
        let id = ResourceHandle::from(&(*path.to_string_lossy()));
        if let Some(_) = self.textures.get(&id) {
            return Ok(id);
        }

        let label = path.file_name().map_or(None, |s| s.to_str());
        let image = load_image(path, Self::DEFAULT_IMAGE_LOAD_MIP)?;
        let texture = Texture::from_image(device, queue, image, label);
        self.textures.insert(id, texture);
        Ok(id)
    }

    pub fn models(&self) -> impl Iterator<Item = (&ResourceHandle<Model>, &Model)> {
        self.models.iter()
    }
}

pub trait GetResource<R> {
    fn get(&self, handle: ResourceHandle<R>) -> Option<&R>;
}

impl GetResource<Model> for AssetServer {
    fn get(&self, handle: ResourceHandle<Model>) -> Option<&Model> {
        self.models.get(&handle)
    }
}

impl GetResource<Texture> for AssetServer {
    fn get(&self, handle: ResourceHandle<Texture>) -> Option<&Texture> {
        self.textures.get(&handle)
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

fn load_image(uri: &path::PathBuf, mip: u32) -> anyhow::Result<image::DynamicImage> {
    let image = image::open(uri)?;
    let (width, height) = image.dimensions();
    let factor = 1 << mip;
    Ok(image.resize(
        width / factor,
        height / factor,
        image::imageops::FilterType::Nearest,
    ))
}

pub(crate) struct GltfAsset {
    document: gltf::Document,
}

impl GltfAsset {
    pub fn document(&self) -> &gltf::Document {
        &self.document
    }

    pub fn images(&self) -> Vec<gltf::image::Image<'_>> {
        self.document.images().collect()
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
