use crate::{Camera, MeshHandle, MeshInstance, asset::load_mere_asset, mesh::MeshHandleID};
use mere_math::Transform;
use mere_mesh::Mesh;
use slotmap::SlotMap;
use std::{
    collections::HashMap,
    rc::{Rc, Weak},
};

#[derive(Clone, Debug)]
pub enum SceneObject {
    Mesh(MeshInstance),
    Camera(Camera),
}

impl SceneObject {
    pub fn mesh(handle: MeshHandle, transform: Transform) -> Self {
        Self::Mesh(MeshInstance { handle, transform })
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
            SceneObject::Mesh(mesh_instance) => &mut mesh_instance.transform,
            SceneObject::Camera(camera) => &mut camera.transform,
        }
    }
}

impl TryFrom<&SceneObject> for MeshInstance {
    type Error = &'static str;

    fn try_from(value: &SceneObject) -> Result<Self, Self::Error> {
        match value {
            SceneObject::Mesh(mesh_instance) => Ok(mesh_instance.clone()),
            _ => Err("object is not mesh instance"),
        }
    }
}

type SceneObjectHandle = slotmap::DefaultKey;

#[derive(Clone, Debug, Default)]
pub struct Scene {
    meshes: HashMap<MeshHandleID, (Mesh, Weak<()>)>,
    objects: SlotMap<SceneObjectHandle, SceneObject>,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            meshes: HashMap::new(),
            objects: SlotMap::new(),
        }
    }

    pub fn add_mesh(&mut self, path: &str) -> anyhow::Result<MeshHandle> {
        let id = MeshHandle::id_from_path(&path);
        if let Some((_, weak)) = self.meshes.get(&id) {
            if let Some(rc) = weak.upgrade() {
                return Ok(MeshHandle {
                    id,
                    _ref_counter: rc,
                });
            }
        }

        let mere_asset = load_mere_asset(path)?;
        let counter = Rc::new(());
        self.meshes
            .insert(id, (mere_asset.mesh(), Rc::downgrade(&counter)));
        Ok(MeshHandle {
            id,
            _ref_counter: counter,
        })
    }

    pub fn remove_mesh(&mut self, handle: MeshHandle) {
        if handle.use_count() <= 2 {
            // 2 because: 1 for this function argument, 1 for the Scene's internal tracking
            self.meshes.remove(&handle.id);
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

    pub fn get_mesh(&self, id: MeshHandleID) -> Option<&Mesh> {
        self.meshes.get(&id).map(|(mesh, _)| mesh)
    }

    pub fn meshes(&self) -> impl Iterator<Item = (MeshHandleID, &Mesh)> {
        self.meshes
            .iter()
            .map(|(handle, (mesh, _))| (*handle, mesh))
    }
}
