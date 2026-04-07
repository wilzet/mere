use crate::{
    Camera,
    asset::{AssetServer, GetResource, load_gltf_asset, load_mere_asset},
    handle::ResourceHandle,
    material::Material,
    model::Model,
};
use mere_common::ASSET_DIR;
use mere_math::{Quat, Transform};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use slotmap::SlotMap;
use std::path;

#[derive(Clone, Copy, Debug)]
pub struct SceneObject {
    model_handle: ResourceHandle<Model>,
    transform: Transform,
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

    pub fn transform(&mut self) -> &mut Transform {
        &mut self.transform
    }
}

pub type SceneObjectHandle = slotmap::DefaultKey;

#[derive(Clone, Debug)]
pub struct Scene {
    objects: SlotMap<SceneObjectHandle, SceneObject>,
    cameras: Vec<Camera>,
    asset_server: AssetServer,
}

impl Scene {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        Self {
            objects: SlotMap::new(),
            cameras: Vec::new(),
            asset_server: AssetServer::new(device, queue),
        }
    }

    pub fn add_camera(&mut self, camera: Camera) -> anyhow::Result<usize> {
        let id = self.cameras.len();
        self.cameras.push(camera);
        Ok(id)
    }

    pub fn add_model(&mut self, model: Model) -> anyhow::Result<ResourceHandle<Model>> {
        self.asset_server.add_model(model)
    }

    pub async fn add_gltf(
        &mut self,
        path: &str,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        material_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> anyhow::Result<Vec<SceneObjectHandle>> {
        let gltf_asset = load_gltf_asset(path)?;
        let mere_asset = load_mere_asset(path)?;
        let mut meshes_iter = mere_asset.meshes();

        gltf_asset.images().iter().for_each(|image| {
            let label = match image.source() {
                gltf::image::Source::View { .. } => {
                    mere_log::error!("Image is buffer view");
                    return;
                }
                gltf::image::Source::Uri { uri, .. } => uri,
            };

            mere_log::info!("Processing {label:?}");

            let image_path = path::PathBuf::from(ASSET_DIR).join(&path).join(label);
            if let Err(err) = self.asset_server.add_texture(&image_path, device, queue) {
                mere_log::error!("{err}");
            }
        });

        let materials = gltf_asset
            .document()
            .materials()
            .filter_map(|material| {
                let default_name = format!("{path}_mat_{}", material.index().unwrap_or(0));
                let name = material.name().unwrap_or(&default_name);

                match Material::from_gltf_material(
                    name,
                    material,
                    device,
                    material_bind_group_layout,
                    &self.asset_server,
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
            device,
            material_bind_group_layout,
            &self.asset_server,
        );

        for model in gltf_asset.document().meshes() {
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

                    mere_mesh::Mesh::from_mere_mesh(name, mere_mesh, device)
                        .with_material(local_material_id)
                })
                .collect();

            let model_materials = used_materials
                .into_iter()
                .map(|id| materials.get(id).unwrap_or(&default_material).clone())
                .collect();

            self.add_model(Model::new(name, meshes, model_materials))?;
        }

        let object_handles = gltf_asset
            .document()
            .nodes()
            .filter_map(|node| {
                if let Some(model) = node.mesh() {
                    let default_name = format!("{path}_model_{}", model.index());
                    let name = model.name().unwrap_or(&default_name);

                    let model_handle = ResourceHandle::from(name);
                    let (translation, rotation, scale) = node.transform().decomposed();
                    let transform = Transform {
                        translation: translation.into(),
                        rotation: Quat::from_vec4(rotation.into()),
                        scale: scale.into(),
                    };

                    Some(self.add_object(SceneObject::new(model_handle, transform)))
                } else {
                    None
                }
            })
            .collect();

        mere_log::info!("Loaded {path}");

        Ok(object_handles)
    }

    pub fn add_object(&mut self, object: SceneObject) -> SceneObjectHandle {
        self.objects.insert(object)
    }

    pub fn remove_object(&mut self, handle: SceneObjectHandle) {
        self.objects.remove(handle);
    }

    pub fn get_object(&self, handle: SceneObjectHandle) -> Option<&SceneObject> {
        self.objects.get(handle)
    }

    pub fn get_object_mut(&mut self, handle: SceneObjectHandle) -> Option<&mut SceneObject> {
        self.objects.get_mut(handle)
    }

    pub fn get_model(&self, id: ResourceHandle<Model>) -> Option<&Model> {
        self.asset_server.get(id)
    }

    pub fn models(&self) -> impl Iterator<Item = (&ResourceHandle<Model>, &Model)> {
        self.asset_server.models()
    }
}
