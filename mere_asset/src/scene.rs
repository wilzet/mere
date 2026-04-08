use crate::{
    Camera,
    asset::{
        AssetEvent, AssetServer, Atomic, DefaultResource, GltfAsset, Resource, load_gltf_asset,
        load_mere_asset, load_texture,
    },
    handle::ResourceHandle,
    material::Material,
    model::{Mesh, Model},
};
use mere_common::ASSET_DIR;
use mere_math::{Quat, Transform};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use slotmap::DenseSlotMap;
use std::path;

#[derive(Clone, Copy, Debug)]
pub struct SceneObject {
    model_handle: ResourceHandle<Model>,
    pub transform: Transform,
}

impl SceneObject {
    pub fn new(handle: ResourceHandle<Model>, transform: Transform) -> Self {
        Self {
            model_handle: handle,
            transform,
        }
    }

    pub fn handle(&self) -> ResourceHandle<Model> {
        self.model_handle
    }
}

pub type SceneObjectHandle = slotmap::DefaultKey;

#[derive(Clone, Debug)]
pub struct Scene {
    objects: DenseSlotMap<SceneObjectHandle, SceneObject>,
    cameras: Vec<Camera>,
    asset_server: AssetServer,
}

impl Scene {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        Self {
            objects: DenseSlotMap::new(),
            cameras: Vec::new(),
            asset_server: AssetServer::new(device, queue),
        }
    }

    pub fn add_camera(&mut self, camera: Camera) -> anyhow::Result<usize> {
        let id = self.cameras.len();
        self.cameras.push(camera);
        Ok(id)
    }

    pub fn load_gltf(
        &mut self,
        path: &str,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> anyhow::Result<Vec<SceneObjectHandle>> {
        let gltf_asset = load_gltf_asset(path)?;
        let doc = gltf_asset.document();

        let asset_server_inner = self.asset_server.clone();
        let device = device.clone();
        let queue = queue.clone();

        let object_handles = doc
            .nodes()
            .filter_map(|node| {
                let model = node.mesh()?;
                let default_name = format!("{path}_model_{}", model.index());
                let name = model.name().unwrap_or(&default_name);

                let model_handle = ResourceHandle::from(name);
                let (translation, rotation, scale) = node.transform().decomposed();
                let transform = Transform {
                    translation: translation.into(),
                    rotation: Quat::from_vec4(rotation.into()),
                    scale: scale.into(),
                };

                asset_server_inner.reserve_handle(model_handle);

                Some(self.add_object(SceneObject::new(model_handle, transform)))
            })
            .collect();

        let path_string = path.to_string();
        std::thread::spawn(move || {
            if let Err(err) =
                background_load_task(&path_string, gltf_asset, device, queue, asset_server_inner)
            {
                mere_log::error!("Async load failed for {path_string}: {err}");
            }
        });

        mere_log::info!("Started background load for {path}");
        Ok(object_handles)
    }

    pub fn process_asset_event(&mut self, device: &wgpu::Device) {
        while let Ok(event) = self.asset_server.try_recv() {
            match event {
                AssetEvent::ModelReady(_) => (),
                AssetEvent::TextureReady(_) => (),
                AssetEvent::MaterialReady(handle) => {
                    self.asset_server.update(handle, |mat| {
                        match mat.finish(device, &self.asset_server) {
                            Ok(_) => (),
                            Err(err) => mere_log::error!("{err} in {}", mat.name),
                        }
                    });
                }
            }
        }
    }

    pub fn add_object(&mut self, object: SceneObject) -> SceneObjectHandle {
        self.objects.insert(object)
    }

    pub fn remove_object(&mut self, handle: SceneObjectHandle) {
        self.objects.remove(handle);
    }

    pub fn objects(&self) -> impl Iterator<Item = &SceneObject> {
        self.objects.values()
    }

    pub fn get_object(&self, handle: SceneObjectHandle) -> Option<&SceneObject> {
        self.objects.get(handle)
    }

    pub fn get_object_mut(&mut self, handle: SceneObjectHandle) -> Option<&mut SceneObject> {
        self.objects.get_mut(handle)
    }

    pub fn get_model(&self, id: ResourceHandle<Model>) -> Option<Atomic<Model>> {
        self.asset_server.get(id)
    }

    pub fn get_material(&self, id: ResourceHandle<Material>) -> Atomic<Material> {
        self.asset_server
            .get_with_default(id, Material::DEFAULT_MATERIAL_ID)
    }
}

fn background_load_task(
    path: &str,
    gltf: GltfAsset,
    device: wgpu::Device,
    queue: wgpu::Queue,
    mut asset_server: AssetServer,
) -> anyhow::Result<()> {
    let mere_asset = load_mere_asset(&path)?;
    let mut meshes_iter = mere_asset.meshes();

    for model in gltf.document().meshes() {
        let default_name = format!("{path}_model_{}", model.index());
        let name = model.name().unwrap_or(&default_name);

        let meshes = model
            .primitives()
            .zip(meshes_iter.by_ref())
            .map(|(primitive, mere_mesh)| {
                let material = match primitive.material().name() {
                    Some(name) => ResourceHandle::from(name),
                    None => Material::DEFAULT_MATERIAL_ID,
                };

                Mesh::from_mere_mesh(name, mere_mesh, &device).with_material(material)
            })
            .collect();

        asset_server.add(Model::new(name, meshes));
    }

    gltf.images().par_iter().for_each(|image| {
        let mut asset_server_inner = asset_server.clone();

        let label = match image.source() {
            gltf::image::Source::View { .. } => {
                mere_log::error!("Unsupported image source");
                return;
            }
            gltf::image::Source::Uri { uri, .. } => uri,
        };

        let image_path = path::PathBuf::from(ASSET_DIR).join(&path).join(label);
        let texture = match load_texture(&image_path, &device, &queue) {
            Ok(tex) => tex,
            Err(err) => {
                mere_log::error!("{err}");
                return;
            }
        };

        asset_server_inner.add(texture);
    });

    gltf.materials().par_iter().for_each(|material| {
        let mut asset_server_inner = asset_server.clone();

        let default_name = format!("{path}_mat_{}", material.index().unwrap_or(0));
        let name = material.name().unwrap_or(&default_name);

        let material =
            match Material::from_gltf_material(name, material, &device, &asset_server_inner) {
                Ok(mat) => mat,
                Err(err) => {
                    mere_log::error!("{err}");
                    return;
                }
            };

        asset_server_inner.add(material);
    });

    mere_log::success!("Loaded {path}");

    Ok(())
}
