use crate::{Camera, MeshHandle, MeshInstance};
use mere_math::Transform;
use mere_mesh::Mesh;
use slotmap::SlotMap;
use std::{collections::HashMap, rc::Rc};

#[derive(Clone, Copy, Debug)]
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

type SceneObjectHandle = slotmap::DefaultKey;

#[derive(Clone, Debug, Default)]
pub struct Scene {
    meshes: HashMap<MeshHandle, Rc<Mesh>>,
    objects: SlotMap<SceneObjectHandle, SceneObject>,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            meshes: HashMap::new(),
            objects: SlotMap::new(),
        }
    }

    pub fn add_mesh(&mut self, mesh: Mesh) -> MeshHandle {
        let handle = MeshHandle::from_mesh(&mesh);
        if !self.meshes.contains_key(&handle) {
            self.meshes.insert(handle, Rc::new(mesh));
        }

        handle
    }

    pub fn remove_mesh(&mut self, handle: MeshHandle) {
        self.meshes.remove(&handle);
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

    pub fn get_object(&self, handle: SceneObjectHandle) -> Option<SceneObject> {
        self.objects.get(handle).copied()
    }
}
