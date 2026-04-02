use crate::{
    Camera, ModelInstance,
    asset::{load_gltf_asset, load_mere_asset},
    handle::{ResourceHandle, ResourceHandleID},
};
use mere_math::{Quat, Transform};
use mere_mesh::{Material, Model};
use slotmap::SlotMap;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug)]
pub enum SceneObject {
    Model(ModelInstance),
    Camera(Camera),
}

impl SceneObject {
    pub fn model(handle: ResourceHandle, transform: Transform) -> Self {
        Self::Model(ModelInstance { handle, transform })
    }

    pub fn camera(fov_y: f32, aspect: f32, far: f32, near: f32, transform: Transform) -> Self {
        Self::Camera(Camera {
            fov_y,
            aspect,
            near,
            far,
            transform,
        })
    }

    pub fn transform(&mut self) -> &mut Transform {
        match self {
            SceneObject::Model(model) => &mut model.transform,
            SceneObject::Camera(camera) => &mut camera.transform,
        }
    }
}

pub type SceneObjectHandle = slotmap::DefaultKey;

#[derive(Clone, Debug, Default)]
pub struct Scene {
    models: HashMap<ResourceHandle, Model>,
    objects: SlotMap<SceneObjectHandle, SceneObject>,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            models: HashMap::new(),
            objects: SlotMap::new(),
        }
    }

    pub fn add_model(&mut self, model: Model) -> anyhow::Result<ResourceHandle> {
        let id = ResourceHandle::from_name(model.name());
        if let Some(_) = self.models.get(&id) {
            return Ok(id);
        }

        self.models.insert(id, model);
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
        let mere_asset = load_mere_asset(path)?;
        let mut meshes_iter = mere_asset.meshes();

        let textures = gltf_asset
            .images()
            .iter()
            .enumerate()
            .map(|(i, image)| {
                let label = gltf_asset
                    .document()
                    .images()
                    .nth(i)
                    .and_then(|img| img.name());
                mere_mesh::Texture::from_bytes(
                    device,
                    queue,
                    &image.pixels,
                    image.width,
                    image.height,
                    label,
                )
            })
            .collect::<Vec<_>>();

        let materials_iter = gltf_asset.document().materials().map(|material| {
            let default_name = format!("{path}_mat_{}", material.index().unwrap_or(0));
            let name = material.name().unwrap_or(&default_name);

            Material::from_gltf_material(
                name,
                material,
                device,
                queue,
                material_bind_group_layout,
                &textures,
            )
        });

        let materials;
        if materials_iter.len() == 0 {
            materials = vec![Material::new(
                &format!("{path}_mat_default"),
                [1.0; 4],
                device,
                queue,
                material_bind_group_layout,
            )]
        } else {
            materials = materials_iter.collect();
        }

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
                .map(|id| materials[id].clone())
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

                    let model_handle = ResourceHandle::from_name(name);
                    let (translation, rotation, scale) = node.transform().decomposed();
                    let transform = Transform {
                        translation: translation.into(),
                        rotation: Quat::from_vec4(rotation.into()),
                        scale: scale.into(),
                    };

                    Some(self.add_object(SceneObject::model(model_handle, transform)))
                } else {
                    None
                }
            })
            .collect();

        Ok(object_handles)
    }

    pub fn add_object(&mut self, object: SceneObject) -> SceneObjectHandle {
        self.objects.insert(object)
    }

    pub fn remove_object(&mut self, handle: SceneObjectHandle) {
        self.objects.remove(handle);
    }

    pub fn cameras(&self) -> impl Iterator<Item = Camera> {
        self.objects.iter().filter_map(|(_, obj)| {
            if let &SceneObject::Camera(cam) = obj {
                Some(cam)
            } else {
                None
            }
        })
    }

    pub fn get_object(&self, handle: SceneObjectHandle) -> Option<&SceneObject> {
        self.objects.get(handle)
    }

    pub fn get_object_mut(&mut self, handle: SceneObjectHandle) -> Option<&mut SceneObject> {
        self.objects.get_mut(handle)
    }

    pub fn get_model(&self, id: ResourceHandleID) -> Option<&Model> {
        self.models.get(&id.into())
    }

    pub fn models(&self) -> impl Iterator<Item = (&ResourceHandle, &Model)> {
        self.models.iter()
    }
}
