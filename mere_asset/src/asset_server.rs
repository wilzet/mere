use crate::{
    Material, Model, Texture,
    asset::Asset,
    handle::{ResourceHandle, UntypedHandle},
};
use crossbeam::channel::{Receiver, Sender, unbounded};
use parking_lot::RwLock;
use std::{collections::HashMap, sync::Arc};

pub(crate) type Atomic<T> = Arc<RwLock<T>>;
type AtomicMap<K, V> = Atomic<HashMap<K, V>>;
type ResourceMap<R> = AtomicMap<ResourceHandle<R>, AssetState<R>>;

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
    Ready(UntypedHandle),
}

#[derive(Clone)]
pub(crate) struct AssetServer {
    models: ResourceMap<Model>,
    textures: ResourceMap<Texture>,
    materials: ResourceMap<Material>,
    dependency_listeners:
        AtomicMap<UntypedHandle, Vec<Box<dyn Fn(&Self, &wgpu::Device) + Send + Sync>>>,
    event_tx: Sender<AssetEvent>,
    event_rx: Receiver<AssetEvent>,
}

impl std::fmt::Debug for AssetServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AssetServer")
            .field("models", &self.models)
            .field("textures", &self.textures)
            .field("materials", &self.materials)
            .field("listeners", &"<listeners>")
            .field("event_tx", &self.event_tx)
            .field("event_rx", &self.event_rx)
            .finish()
    }
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
            dependency_listeners: Arc::new(RwLock::new(HashMap::new())),
            event_tx: tx,
            event_rx: rx,
        };

        asset_server.materials.write().insert(
            Material::DEFAULT_MATERIAL_ID,
            AssetState::new(Some(Material::default_material(device, &asset_server))),
        );

        asset_server
    }

    pub fn subscribe<R: Asset + Send + Sync + 'static, F>(&self, handle: ResourceHandle<R>, f: F)
    where
        F: Fn(&Self, &wgpu::Device) + Send + Sync + 'static,
    {
        let untyped_handle = handle.into();
        let mut listeners = self.dependency_listeners.write();
        listeners
            .entry(untyped_handle)
            .or_default()
            .push(Box::new(move |assets, device| f(assets, device)));
    }

    pub fn dispatch_ready(&self, handle: UntypedHandle, device: &wgpu::Device) {
        if let Some(listeners) = self.dependency_listeners.write().remove(&handle) {
            for listener in listeners {
                listener(self, device);
            }
        }
    }

    pub fn try_recv(&self) -> anyhow::Result<AssetEvent> {
        Ok(self.event_rx.try_recv()?)
    }
}

pub trait Resource<R: Asset> {
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

pub trait DefaultResource<R: Asset>: Resource<R> {
    fn get_with_default(&self, handle: ResourceHandle<R>, default: ResourceHandle<R>) -> Atomic<R> {
        match self.get(handle) {
            Some(value) => value,
            None => self.get(default).unwrap(),
        }
    }
}

macro_rules! resource_impl {
    ($resource:ty, $storage:ident, $ident:ident) => {
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

                let _ = self.event_tx.send(AssetEvent::Ready(id.into()));

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

resource_impl!(Model, models, name);
resource_impl!(Texture, textures, label);
resource_impl!(Material, materials, name);

impl DefaultResource<Texture> for AssetServer {}
impl DefaultResource<Material> for AssetServer {}
