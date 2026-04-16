use crate::{
    Material, Model, Texture,
    asset::Asset,
    handle::{ResourceHandle, UntypedHandle},
    texture,
};
use crossbeam::channel::{Receiver, Sender, unbounded};
use parking_lot::RwLock;
use std::{any::TypeId, collections::HashMap, sync::Arc};

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

#[derive(Clone, Debug)]
pub(crate) struct AssetServer {
    pub device: wgpu::Device,
    models: ResourceMap<Model>,
    textures: ResourceMap<Texture>,
    materials: ResourceMap<Material>,
    dependency_listeners: AtomicMap<UntypedHandle, Vec<UntypedHandle>>,
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
                    color.to_vec(),
                    label,
                    texture::TextureOptions::texture(wgpu::TextureFormat::Rgba8Unorm)
                        .with_mag_min_filter(wgpu::FilterMode::Nearest, wgpu::FilterMode::Nearest),
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
            device: device.clone(),
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

    pub fn subscribe<R: Asset + 'static>(
        &self,
        dependency_handle: UntypedHandle,
        handle: ResourceHandle<R>,
    ) {
        let mut listeners = self.dependency_listeners.write();
        listeners
            .entry(dependency_handle)
            .or_default()
            .push(handle.into());
    }

    pub fn dispatch_ready(&self, handle: UntypedHandle) {
        if let Some(listeners) = self.dependency_listeners.write().remove(&handle) {
            for listener in listeners {
                self.finish_untyped(listener);
            }
        }
    }

    pub fn finish_untyped(&self, handle: UntypedHandle) {
        let type_id = handle.type_id();

        if type_id == TypeId::of::<Material>() {
            let h = ResourceHandle::<Material>::new(*handle);
            self.finish(h);
        } else if type_id == TypeId::of::<Texture>() {
            let h = ResourceHandle::<Texture>::new(*handle);
            self.finish(h);
        } else if type_id == TypeId::of::<Model>() {
            let h = ResourceHandle::<Model>::new(*handle);
            self.finish(h);
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
    fn finish(&self, handle: ResourceHandle<R>);
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

                for dependent in value.dependencies() {
                    self.subscribe(dependent, id);
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

            fn finish(&self, handle: ResourceHandle<$resource>) {
                if let Some(resource) = self.get(handle) {
                    let mut r = resource.write();
                    r.finish(self);
                }
            }
        }
    };
}

resource_impl!(Model, models, name);
resource_impl!(Texture, textures, label);
resource_impl!(Material, materials, name);

impl DefaultResource<Texture> for AssetServer {}
impl DefaultResource<Material> for AssetServer {}
