use crate::{
    asset::Asset,
    handle::{ResourceHandle, UntypedHandle},
    material::Material,
    texture::{Texture, TextureOptions},
};
use crossbeam::channel::{Receiver, Sender, unbounded};
use mere_mesh::MeshletMesh;
use parking_lot::RwLock;
use std::{any::TypeId, collections::HashMap, sync::Arc};

pub type Shared<T> = Arc<RwLock<T>>;
type SharedMap<K, V> = Shared<HashMap<K, V>>;
type ResourceMap<R> = SharedMap<ResourceHandle<R>, AssetState<R>>;

#[derive(Clone, Debug)]
enum AssetState<R> {
    Loading,
    Ready(Shared<R>),
}

impl<R> AssetState<R> {
    fn new(has_resource: Option<R>) -> Self {
        match has_resource {
            Some(value) => Self::Ready(RwLock::new(value).into()),
            None => Self::Loading,
        }
    }
}

#[derive(Clone, Debug)]
pub enum AssetEvent {
    Ready(UntypedHandle),
    MeshletReady(ResourceHandle<MeshletMesh>),
}

#[derive(Clone, Debug)]
pub struct AssetServer {
    device: wgpu::Device,
    meshlets: ResourceMap<MeshletMesh>,
    textures: ResourceMap<Texture>,
    materials: ResourceMap<Material>,
    dependency_listeners: SharedMap<UntypedHandle, Vec<UntypedHandle>>,
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
                    TextureOptions::texture(wgpu::TextureFormat::Rgba8Unorm)
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
            meshlets: Arc::new(RwLock::new(HashMap::new())),
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

        self.finish_untyped(handle);
    }

    fn finish_untyped(&self, handle: UntypedHandle) {
        let type_id = handle.type_id();

        if type_id == TypeId::of::<Material>() {
            self.finish(ResourceHandle::<Material>::new(*handle));
        } else if type_id == TypeId::of::<Texture>() {
            self.finish(ResourceHandle::<Texture>::new(*handle));
        } else if type_id == TypeId::of::<MeshletMesh>() {
            self.finish(ResourceHandle::<MeshletMesh>::new(*handle));
        }
    }

    pub fn try_recv(&self) -> anyhow::Result<AssetEvent> {
        Ok(self.event_rx.try_recv()?)
    }

    pub fn send(
        &self,
        asset_event: AssetEvent,
    ) -> Result<(), crossbeam::channel::SendError<AssetEvent>> {
        self.event_tx.send(asset_event)
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn pending_dependencies(&self) -> usize {
        self.dependency_listeners.read().len()
    }

    pub fn report_memory(&self) -> (usize, usize, usize) {
        fn asset_size_in_bytes<T: Asset>(state: &AssetState<T>) -> usize {
            if let AssetState::Ready(r) = state {
                r.read().size_in_bytes()
            } else {
                0
            }
        }

        let mesh_size = self.meshlets.read().values().map(asset_size_in_bytes).sum();
        let tex_size = self.textures.read().values().map(asset_size_in_bytes).sum();
        let mat_size = self
            .materials
            .read()
            .values()
            .map(asset_size_in_bytes)
            .sum();

        (mesh_size, tex_size, mat_size)
    }
}

pub trait Resource<R: Asset> {
    fn get(&self, handle: ResourceHandle<R>) -> Option<Shared<R>>;
    fn add(&mut self, value: R) -> ResourceHandle<R>;
    fn reserve_handle(&self, handle: ResourceHandle<R>) -> bool;
    fn finish(&self, handle: ResourceHandle<R>);
}

pub trait DefaultResource<R: Asset>: Resource<R> {
    fn get_with_default(&self, handle: ResourceHandle<R>, default: ResourceHandle<R>) -> Shared<R> {
        match self.get(handle) {
            Some(value) => value,
            None => self.get(default).unwrap(),
        }
    }
}

macro_rules! resource_impl {
    ($resource:ty, $storage:ident, $ident:ident) => {
        impl Resource<$resource> for AssetServer {
            fn get(&self, handle: ResourceHandle<$resource>) -> Option<Shared<$resource>> {
                match self.$storage.read().get(&handle) {
                    Some(AssetState::Ready(value)) => Some(value.clone()),
                    _ => None,
                }
            }

            fn add(&mut self, value: $resource) -> ResourceHandle<$resource> {
                let id = ResourceHandle::from(value.$ident.as_str());
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

                let _ = self.send(AssetEvent::Ready(id.into()));

                id
            }

            fn reserve_handle(&self, handle: ResourceHandle<$resource>) -> bool {
                matches!(
                    self.$storage
                        .write()
                        .entry(handle)
                        .or_insert(AssetState::Loading),
                    AssetState::Ready(_)
                )
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

resource_impl!(MeshletMesh, meshlets, name);
resource_impl!(Texture, textures, label);
resource_impl!(Material, materials, name);

impl DefaultResource<Texture> for AssetServer {}
impl DefaultResource<Material> for AssetServer {}
