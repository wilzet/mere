use crate::{
    asset::{Asset, GltfAsset, load_gltf_asset, load_mere_meshes},
    asset_server::{AssetEvent, AssetServer, DefaultResource, Resource, Shared},
    camera::Camera,
    handle::ResourceHandle,
    material::Material,
    model::Model,
    texture::{MipmapOptions, Texture, TextureOptions},
};
use mere_common::ASSET_DIR;
use mere_math::{Quat, Transform};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use slotmap::DenseSlotMap;
use std::path;

#[derive(Clone, Copy, Debug)]
pub struct SceneObject {
    pub transform: Transform,
    model_handle: ResourceHandle<Model>,
}

impl SceneObject {
    pub fn new(handle: ResourceHandle<Model>, transform: Transform) -> Self {
        Self {
            transform,
            model_handle: handle,
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

    pub fn add_camera(&mut self, camera: Camera) -> usize {
        let id = self.cameras.len();
        self.cameras.push(camera);
        id
    }

    pub fn load_gltf(
        &mut self,
        path: &str,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> anyhow::Result<Vec<SceneObjectHandle>> {
        let gltf_asset = load_gltf_asset(path)?;

        let asset_server_inner = self.asset_server.clone();
        let device = device.clone();
        let queue = queue.clone();

        let object_handles = gltf_asset
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

    pub fn process_asset_event(&mut self) {
        while let Ok(event) = self.asset_server.try_recv() {
            match event {
                AssetEvent::Ready(handle) => self.asset_server.dispatch_ready(handle),
            }
        }
    }

    pub fn add_object(&mut self, object: SceneObject) -> SceneObjectHandle {
        self.objects.insert(object)
    }

    pub fn remove_object(&mut self, handle: SceneObjectHandle) {
        self.objects.remove(handle);
    }

    pub fn objects(&self) -> slotmap::dense::Values<'_, SceneObjectHandle, SceneObject> {
        self.objects.values()
    }

    pub fn cameras(&self) -> std::slice::Iter<'_, Camera> {
        self.cameras.iter()
    }

    pub fn main_camera(&self) -> &Camera {
        &self.cameras[0]
    }

    pub fn main_camera_mut(&mut self) -> &mut Camera {
        &mut self.cameras[0]
    }

    pub fn get_object(&self, handle: SceneObjectHandle) -> Option<&SceneObject> {
        self.objects.get(handle)
    }

    pub fn get_object_mut(&mut self, handle: SceneObjectHandle) -> Option<&mut SceneObject> {
        self.objects.get_mut(handle)
    }

    pub fn get_model(&self, id: ResourceHandle<Model>) -> Option<Shared<Model>> {
        self.asset_server.get(id)
    }

    pub fn get_material(&self, id: ResourceHandle<Material>) -> Shared<Material> {
        self.asset_server
            .get_with_default(id, Material::DEFAULT_MATERIAL_ID)
    }
}

fn background_load_task(
    path: &str,
    gltf_asset: GltfAsset,
    device: wgpu::Device,
    queue: wgpu::Queue,
    mut asset_server: AssetServer,
) -> anyhow::Result<()> {
    let mut mere_meshes = load_mere_meshes(&path)?.into_iter();

    for model in gltf_asset.models() {
        let meshes = mere_meshes
            .by_ref()
            .take(model.primitives().len())
            .collect();
        let model = Model::load((path, model, meshes, &device))?;

        asset_server.add(model);
    }

    for material in gltf_asset.materials() {
        asset_server.add(Material::load((path, material, &device, &asset_server))?);
    }

    const DEFAULT_IMAGE_LOAD_MIP: u32 = 1;
    gltf_asset.images().par_iter().for_each(|image| {
        let uri = match image.source() {
            gltf::image::Source::Uri { uri, .. } => uri,
            gltf::image::Source::View { .. } => {
                mere_log::error!("Unsupported image source");
                return;
            }
        };

        let options = texture_options(uri);
        let image_path = path::PathBuf::from(ASSET_DIR).join(&path).join(uri);

        match Texture::load((
            &image_path,
            &device,
            &queue,
            options,
            DEFAULT_IMAGE_LOAD_MIP,
        )) {
            Ok(tex) => {
                asset_server.clone().add(tex);
            }
            Err(err) => mere_log::error!("Failed to load texture {uri}: {err}"),
        };
    });

    mere_log::success!("Loaded {path}");

    Ok(())
}

fn texture_options(uri: &str) -> TextureOptions {
    const DIFFUSE_ANISOTROPY: u16 = 8;
    const OTHER_ANISOTROPY: u16 = 8;

    let lower = uri.to_lowercase();
    if lower.contains("normal") || (lower.contains("roughness") && lower.contains("metalness")) {
        TextureOptions::texture(wgpu::TextureFormat::Rgba8Unorm)
            .with_mipmap(MipmapOptions::auto(Some(OTHER_ANISOTROPY)))
    } else {
        TextureOptions::texture(wgpu::TextureFormat::Rgba8UnormSrgb)
            .with_mipmap(MipmapOptions::auto(Some(DIFFUSE_ANISOTROPY)))
    }
}
