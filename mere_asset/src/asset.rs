use crate::{handle::ResourceHandle, model::Model, texture::Texture};
use common::{collect_gltf_files, read_mere_file};
use image::GenericImageView;
use mere_common::{ASSET_DIR, PROCESSED_ASSET_DIR};
use mere_mesh::MereMesh;
use parking_lot::RwLock;
use std::{collections::HashMap, path, sync::Arc};

#[derive(Clone, Debug)]
pub enum AssetState<R> {
    Loading,
    Ready(R),
}

type ResourceMap<R> = Arc<RwLock<HashMap<ResourceHandle<R>, AssetState<Arc<R>>>>>;

#[derive(Clone, Debug)]
pub(crate) struct AssetServer {
    models: ResourceMap<Model>,
    textures: ResourceMap<Texture>,
}

impl AssetServer {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let mut textures = HashMap::new();

        Texture::DEFAULT_TEXTURES
            .iter()
            .for_each(|&(id, label, color)| {
                let texture = match Texture::from_bytes(device, queue, 1, 1, &color, label) {
                    Ok(tex) => tex,
                    Err(err) => {
                        mere_log::error!("{err}");
                        return;
                    }
                };

                textures.insert(id, AssetState::Ready(texture.into()));
            });

        Self {
            models: Arc::new(RwLock::new(HashMap::new())),
            textures: Arc::new(RwLock::new(textures)),
        }
    }
}

pub trait Resource<R> {
    fn get(&self, handle: ResourceHandle<R>) -> Option<Arc<R>>;
    fn add(&mut self, value: R) -> anyhow::Result<ResourceHandle<R>>;
    fn reserve_handle(&self, handle: ResourceHandle<R>);
}

impl Resource<Model> for AssetServer {
    fn get(&self, handle: ResourceHandle<Model>) -> Option<Arc<Model>> {
        match self.models.read().get(&handle) {
            Some(AssetState::Ready(model)) => Some(model.clone()),
            _ => None,
        }
    }

    fn add(&mut self, model: Model) -> anyhow::Result<ResourceHandle<Model>> {
        let id = ResourceHandle::from(model.name());
        {
            if let Some(AssetState::Ready(_)) = self.models.read().get(&id) {
                return Ok(id);
            }
        }

        self.models
            .write()
            .insert(id, AssetState::Ready(model.into()));
        Ok(id)
    }

    fn reserve_handle(&self, handle: ResourceHandle<Model>) {
        self.models
            .write()
            .entry(handle)
            .or_insert(AssetState::Loading);
    }
}

impl Resource<Texture> for AssetServer {
    fn get(&self, handle: ResourceHandle<Texture>) -> Option<Arc<Texture>> {
        match self.textures.read().get(&handle) {
            Some(AssetState::Ready(texture)) => Some(texture.clone()),
            _ => None,
        }
    }

    fn add(&mut self, texture: Texture) -> anyhow::Result<ResourceHandle<Texture>> {
        let id = ResourceHandle::from(texture.label());
        {
            if let Some(AssetState::Ready(_)) = self.textures.read().get(&id) {
                return Ok(id);
            }
        }

        self.textures
            .write()
            .insert(id, AssetState::Ready(texture.into()));
        Ok(id)
    }

    fn reserve_handle(&self, handle: ResourceHandle<Texture>) {
        self.textures
            .write()
            .entry(handle)
            .or_insert(AssetState::Loading);
    }
}

const DEFAULT_IMAGE_LOAD_MIP: u32 = 5;

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

pub(crate) fn load_texture(
    path: &path::PathBuf,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> anyhow::Result<Texture> {
    let label = match path.file_name().map_or(None, |s| s.to_str()) {
        Some(name) => name,
        None => anyhow::bail!("No file found in path: {path:?}"),
    };

    let image = load_image(path, DEFAULT_IMAGE_LOAD_MIP)?;
    Ok(Texture::from_image(device, queue, image, label))
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
