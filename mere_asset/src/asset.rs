use crate::{Material, handle::ResourceHandle, model::Model, texture::Texture};
use common::{collect_gltf_files, read_mere_file};
use crossbeam::channel::{Receiver, Sender, unbounded};
use image::GenericImageView;
use mere_common::{ASSET_DIR, PROCESSED_ASSET_DIR};
use mere_mesh::MereMesh;
use parking_lot::RwLock;
use std::{collections::HashMap, path, sync::Arc};

pub(crate) type Atomic<T> = Arc<RwLock<T>>;
type ResourceMap<R> = Atomic<HashMap<ResourceHandle<R>, AssetState<R>>>;

#[derive(Clone, Debug)]
pub enum AssetState<R> {
    Loading,
    Ready(Atomic<R>),
}

impl<R> AssetState<R> {
    pub fn new(has_resource: Option<R>) -> Self {
        match has_resource {
            Some(value) => Self::Ready(RwLock::new(value).into()),
            None => Self::Loading,
        }
    }
}

#[derive(Clone, Debug)]
pub enum AssetEvent {
    ModelReady(ResourceHandle<Model>),
    TextureReady(ResourceHandle<Texture>),
    MaterialReady(ResourceHandle<Material>),
}

#[derive(Clone, Debug)]
pub(crate) struct AssetServer {
    models: ResourceMap<Model>,
    textures: ResourceMap<Texture>,
    materials: ResourceMap<Material>,
    event_tx: Sender<AssetEvent>,
    event_rx: Receiver<AssetEvent>,
}

impl AssetServer {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let textures = Texture::DEFAULT_TEXTURES
            .iter()
            .filter_map(|&(id, label, color)| {
                let texture = match Texture::from_bytes_with_options(
                    device,
                    queue,
                    2,
                    2,
                    &color,
                    label,
                    wgpu::TextureFormat::Rgba8Unorm,
                    wgpu::AddressMode::Repeat,
                    wgpu::FilterMode::Nearest,
                    wgpu::FilterMode::Nearest,
                    wgpu::MipmapFilterMode::Nearest,
                ) {
                    Ok(tex) => tex,
                    Err(err) => {
                        mere_log::error!("{err}");
                        return None;
                    }
                };

                Some((id, AssetState::new(Some(texture))))
            })
            .collect();

        let (tx, rx) = unbounded();

        let asset_server = Self {
            models: Arc::new(RwLock::new(HashMap::new())),
            textures: Arc::new(RwLock::new(textures)),
            materials: Arc::new(RwLock::new(HashMap::new())),
            event_tx: tx,
            event_rx: rx,
        };

        asset_server.materials.write().insert(
            Material::DEFAULT_MATERIAL_ID,
            AssetState::new(Some(Material::default_material(device, &asset_server))),
        );

        asset_server
    }

    pub fn try_recv(&self) -> anyhow::Result<AssetEvent> {
        Ok(self.event_rx.try_recv()?)
    }
}

pub trait Resource<R> {
    fn get(&self, handle: ResourceHandle<R>) -> Option<Atomic<R>>;
    fn add(&mut self, value: R) -> ResourceHandle<R>;
    fn reserve_handle(&self, handle: ResourceHandle<R>);
    fn update<F: FnOnce(&mut R)>(&self, handle: ResourceHandle<R>, f: F) {
        if let Some(resource) = self.get(handle) {
            let mut r = resource.write();
            f(&mut r);
        }
    }
}

pub trait DefaultResource<R>: Resource<R> {
    fn get_with_default(&self, handle: ResourceHandle<R>, default: ResourceHandle<R>) -> Atomic<R> {
        match self.get(handle) {
            Some(value) => value,
            None => self.get(default).unwrap(),
        }
    }
}

macro_rules! resource_impl {
    ($resource:ty, $event:ident, $storage:ident, $ident:ident) => {
        impl Resource<$resource> for AssetServer {
            fn get(&self, handle: ResourceHandle<$resource>) -> Option<Atomic<$resource>> {
                match self.$storage.read().get(&handle) {
                    Some(AssetState::Ready(value)) => Some(value.clone()),
                    _ => None,
                }
            }

            fn add(&mut self, value: $resource) -> ResourceHandle<$resource> {
                let id = ResourceHandle::from(value.$ident());
                {
                    if let Some(AssetState::Ready(_)) = self.$storage.read().get(&id) {
                        return id;
                    }
                }

                self.$storage
                    .write()
                    .insert(id, AssetState::new(Some(value)));

                let _ = self.event_tx.send(AssetEvent::$event(id));

                id
            }

            fn reserve_handle(&self, handle: ResourceHandle<$resource>) {
                self.$storage
                    .write()
                    .entry(handle)
                    .or_insert(AssetState::Loading);
            }
        }
    };
}

resource_impl!(Model, ModelReady, models, name);
resource_impl!(Texture, TextureReady, textures, label);
resource_impl!(Material, MaterialReady, materials, name);

impl DefaultResource<Texture> for AssetServer {}
impl DefaultResource<Material> for AssetServer {}

const DEFAULT_IMAGE_LOAD_MIP: u32 = 3;

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
    Ok(Texture::from_image(
        device,
        queue,
        image,
        label,
        wgpu::TextureFormat::Rgba8UnormSrgb,
    )?)
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

    pub fn images(&self) -> Vec<gltf::Image<'_>> {
        self.document.images().collect()
    }

    pub fn materials(&self) -> Vec<gltf::Material> {
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
