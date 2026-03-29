use crate::{Camera, ModelHandle, ModelInstance, asset::load_mere_asset, model::ModelHandleID};
use mere_math::Transform;
use mere_mesh::Model;
use slotmap::SlotMap;
use std::{
    collections::HashMap,
    rc::{Rc, Weak},
};

#[derive(Clone, Debug)]
pub enum SceneObject {
    Model(ModelInstance),
    Camera(Camera),
}

impl SceneObject {
    pub fn model(handle: ModelHandle, transform: Transform) -> Self {
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

impl TryFrom<&SceneObject> for ModelInstance {
    type Error = &'static str;

    fn try_from(value: &SceneObject) -> Result<Self, Self::Error> {
        match value {
            SceneObject::Model(model) => Ok(model.clone()),
            _ => Err("object is not model instance"),
        }
    }
}

type SceneObjectHandle = slotmap::DefaultKey;

#[derive(Clone, Debug, Default)]
pub struct Scene {
    models: HashMap<ModelHandleID, (Model, Weak<()>)>,
    objects: SlotMap<SceneObjectHandle, SceneObject>,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            models: HashMap::new(),
            objects: SlotMap::new(),
        }
    }

    pub fn add_model(&mut self, path: &str) -> anyhow::Result<ModelHandle> {
        let id = ModelHandle::id_from_path(&path);
        if let Some((_, weak)) = self.models.get(&id) {
            if let Some(rc) = weak.upgrade() {
                return Ok(ModelHandle {
                    id,
                    _ref_counter: rc,
                });
            }
        }

        let mere_asset = load_mere_asset(path)?;
        let counter = Rc::new(());
        self.models
            .insert(id, (mere_asset.model(), Rc::downgrade(&counter)));
        Ok(ModelHandle {
            id,
            _ref_counter: counter,
        })
    }

    pub fn remove_model(&mut self, handle: ModelHandle) {
        if handle.use_count() <= 2 {
            // 2 because: 1 for this function argument, 1 for the Scene's internal tracking
            self.models.remove(&handle.id);
        }
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

    pub fn get_model(&self, id: ModelHandleID) -> Option<&Model> {
        self.models.get(&id).map(|(model, _)| model)
    }

    pub fn models(&self) -> impl Iterator<Item = (ModelHandleID, &Model)> {
        self.models
            .iter()
            .map(|(handle, (model, _))| (*handle, model))
    }
}
