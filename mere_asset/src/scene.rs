use crate::{
    Camera,
    asset::{AssetServer, GltfAsset, Resource, load_gltf_asset, load_mere_asset, load_texture},
    handle::ResourceHandle,
    material::Material,
    model::Model,
};
use mere_common::ASSET_DIR;
use mere_math::{Quat, Transform};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use slotmap::DenseSlotMap;
use std::{path, sync::Arc};

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

    pub fn add_gltf(
        &mut self,
        path: &str,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        material_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> anyhow::Result<Vec<SceneObjectHandle>> {
        let gltf_asset = load_gltf_asset(path)?;
        let doc = gltf_asset.document();

        let asset_server_inner = self.asset_server.clone();
        let device = device.clone();
        let queue = queue.clone();
        let material_layout = material_bind_group_layout.clone();

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
            if let Err(err) = background_load_task(
                &path_string,
                gltf_asset,
                device,
                queue,
                material_layout,
                asset_server_inner,
            ) {
                mere_log::error!("Async load failed for {path_string}: {err}");
            }
        });

        mere_log::info!("Started background load for {path}");
        Ok(object_handles)
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

    pub fn get_model(&self, id: ResourceHandle<Model>) -> Option<Arc<Model>> {
        self.asset_server.get(id)
    }
}

fn background_load_task(
    path: &str,
    gltf: GltfAsset,
    device: wgpu::Device,
    queue: wgpu::Queue,
    material_bind_group_layout: wgpu::BindGroupLayout,
    mut asset_server: AssetServer,
) -> anyhow::Result<()> {
    let mere_asset = load_mere_asset(&path)?;
    let mut meshes_iter = mere_asset.meshes();

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
        if let Err(err) = asset_server_inner.add(texture) {
            mere_log::error!("{err}");
        }
    });

    let materials = gltf
        .document()
        .materials()
        .filter_map(|material| {
            let default_name = format!("{path}_mat_{}", material.index().unwrap_or(0));
            let name = material.name().unwrap_or(&default_name);

            match Material::from_gltf_material(
                name,
                material,
                &device,
                &material_bind_group_layout,
                &asset_server,
            ) {
                Ok(mat) => Some(mat),
                Err(err) => {
                    mere_log::error!("{err}");
                    None
                }
            }
        })
        .collect::<Vec<_>>();

    let default_material = Material::new(
        &format!("{path}_mat_default"),
        [1.0; 4],
        &device,
        &material_bind_group_layout,
        &asset_server,
    );

    for model in gltf.document().meshes() {
        let default_name = format!("{path}_model_{}", model.index());
        let name = model.name().unwrap_or(&default_name);

        let mut used_materials = Vec::new();

        let meshes = model
            .primitives()
            .zip(meshes_iter.by_ref())
            .map(|(primitive, mere_mesh)| {
                let material_id = primitive.material().index().unwrap_or(0);
                if !used_materials.contains(&material_id) {
                    used_materials.push(material_id);
                }

                let local_material_id = used_materials
                    .iter()
                    .position(|&x| x == material_id)
                    .unwrap_or(0);

                mere_mesh::Mesh::from_mere_mesh(name, mere_mesh, &device)
                    .with_material(local_material_id)
            })
            .collect();

        let model_materials = used_materials
            .into_iter()
            .map(|id| materials.get(id).unwrap_or(&default_material).clone())
            .collect();

        asset_server.add(Model::new(name, meshes, model_materials))?;
    }

    mere_log::success!("Loaded {path}");

    Ok(())
}
