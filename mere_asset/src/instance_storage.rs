use crate::{gpu_buffer::GpuStorageBuffer, handle::ResourceHandle, material::Material};
use mere_math::Transform;
use mere_mesh::MeshletMesh;
use slotmap::DenseSlotMap;

pub type InstanceHandle = slotmap::DefaultKey;

#[derive(Debug)]
pub struct InstanceStorage {
    pub scene_instance_count: u32,
    instances: DenseSlotMap<InstanceHandle, Instance>,
    pub instance_uniforms: GpuStorageBuffer<Vec<MeshUniform>>,
    pub instance_material_ids: GpuStorageBuffer<Vec<u32>>,
}

impl InstanceStorage {
    pub fn new() -> Self {
        Self {
            scene_instance_count: 0,
            instances: DenseSlotMap::new(),
            instance_uniforms: GpuStorageBuffer::new(Some("meshlet_instance_uniforms"), Vec::new()),
            instance_material_ids: GpuStorageBuffer::new(
                Some("meshlet_instance_material_ids"),
                Vec::new(),
            ),
        }
    }

    pub fn iter(&self) -> slotmap::dense::Values<'_, InstanceHandle, Instance> {
        self.instances.values()
    }

    pub fn get(&self, handle: InstanceHandle) -> Option<&Instance> {
        self.instances.get(handle)
    }

    pub fn get_mut(&mut self, handle: InstanceHandle) -> Option<&mut Instance> {
        self.instances.get_mut(handle)
    }

    pub fn add_instance(
        &mut self,
        instance: Instance,
        material: ResourceHandle<Material>,
    ) -> InstanceHandle {
        let mesh_uniform = MeshUniform::new(instance.transform);

        let handle = self.instances.insert(instance);
        self.instance_uniforms.get_mut().push(mesh_uniform);
        self.instance_material_ids.get_mut().push(*material as u32);

        self.scene_instance_count += 1;

        handle
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub struct Instance {
    pub transform: Transform,
    pub meshlet: ResourceHandle<MeshletMesh>,
}

impl Instance {
    pub fn new(transform: Transform, meshlet_mesh_handle: ResourceHandle<MeshletMesh>) -> Self {
        Self {
            transform,
            meshlet: meshlet_mesh_handle,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Default, Debug)]
pub struct MeshUniform {
    pub model: [[f32; 4]; 4],
    pub previous_model: [[f32; 4]; 4],
    pub normal: [[f32; 4]; 3],
}

impl MeshUniform {
    pub fn new(transform: Transform) -> Self {
        let normal = {
            let m = transform.rotation_matrix().to_cols_array_2d();
            [
                [m[0][0], m[0][1], m[0][2], 0.0],
                [m[1][0], m[1][1], m[1][2], 0.0],
                [m[2][0], m[2][1], m[2][2], 0.0],
            ]
        };

        Self {
            model: transform.trs().to_cols_array_2d(),
            previous_model: transform.trs().to_cols_array_2d(),
            normal,
        }
    }
}
