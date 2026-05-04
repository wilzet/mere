use crate::{
    gpu_buffer::{GpuBufferable, GpuStorageBuffer},
    handle::ResourceHandle,
    material::Material,
};
use mere_math::Transform;
use mere_mesh::{Aabb, MeshletMesh};
use slotmap::DenseSlotMap;

pub type InstanceHandle = slotmap::DefaultKey;

#[derive(Debug)]
pub struct InstanceStorage {
    pub scene_instance_count: u32,
    instances: DenseSlotMap<InstanceHandle, Instance>,
    pub instance_uniforms: GpuStorageBuffer<Vec<MeshUniform>>,
    pub instance_aabbs: GpuStorageBuffer<Vec<Aabb>>,
    pub instance_meshlet_offsets: GpuStorageBuffer<Vec<u32>>,
    pub instance_meshlet_counts: GpuStorageBuffer<Vec<u32>>,
    pub instance_material_ids: GpuStorageBuffer<Vec<u32>>,
}

impl InstanceStorage {
    pub fn new() -> Self {
        Self {
            scene_instance_count: 0,
            instances: DenseSlotMap::new(),
            instance_uniforms: GpuStorageBuffer::new(Some("meshlet_instance_uniforms"), Vec::new()),
            instance_aabbs: GpuStorageBuffer::new(Some("meshlet_instance_aabbs"), Vec::new()),
            instance_meshlet_offsets: GpuStorageBuffer::new(
                Some("meshlet_instance_meshlet_offsets"),
                Vec::new(),
            ),
            instance_meshlet_counts: GpuStorageBuffer::new(
                Some("meshlet_instance_meshlet_counts"),
                Vec::new(),
            ),
            instance_material_ids: GpuStorageBuffer::new(
                Some("meshlet_instance_material_ids"),
                Vec::new(),
            ),
        }
    }

    pub fn iter(&self) -> slotmap::dense::Values<'_, InstanceHandle, Instance> {
        self.instances.values()
    }

    pub fn iter_mut(&mut self) -> slotmap::dense::ValuesMut<'_, InstanceHandle, Instance> {
        self.instances.values_mut()
    }

    pub fn get(&self, handle: InstanceHandle) -> Option<&Instance> {
        self.instances.get(handle)
    }

    pub fn get_mut(&mut self, handle: InstanceHandle) -> Option<&mut Instance> {
        self.instances.get_mut(handle)
    }

    pub fn add_instance(&mut self, instance: Instance) -> InstanceHandle {
        self.instances.insert(instance)
    }

    pub fn count_clusters(&self) -> usize {
        self.instances
            .iter()
            .fold(0, |acc, (_, i)| i.meshlet_count + acc) as usize
    }

    pub fn build_instance_buffers(&mut self) {
        for (_, instance) in self.instances.iter() {
            let mesh_uniform = MeshUniform::new(instance.transform);

            self.instance_uniforms.get_mut().push(mesh_uniform);
            self.instance_aabbs.get_mut().push(instance.aabb);
            self.instance_meshlet_offsets
                .get_mut()
                .push(instance.meshlet_offset);
            self.instance_meshlet_counts
                .get_mut()
                .push(instance.meshlet_count);
            self.instance_material_ids
                .get_mut()
                .push(*instance.material as u32);
        }

        self.scene_instance_count = self.instances.len() as u32;
    }

    pub fn reset(&mut self) {
        self.scene_instance_count = 0;

        self.instance_uniforms.get_mut().clear();
        self.instance_aabbs.get_mut().clear();
        self.instance_meshlet_offsets.get_mut().clear();
        self.instance_meshlet_counts.get_mut().clear();
        self.instance_material_ids.get_mut().clear();
    }

    pub fn report_memory_host(&self) -> usize {
        let mut total = 0;

        total += size_of::<Self>();
        total += self.instances.len() * size_of::<Instance>();
        total
    }

    pub fn report_memory_device(&self) -> usize {
        let mut total = 0;

        total += self.instance_uniforms.get().size_in_bytes();
        total += self.instance_aabbs.get().size_in_bytes();
        total += self.instance_meshlet_offsets.get().size_in_bytes();
        total += self.instance_meshlet_counts.get().size_in_bytes();
        total += self.instance_material_ids.get().size_in_bytes();
        total
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Instance {
    pub transform: Transform,
    pub aabb: Aabb,
    pub meshlet: ResourceHandle<MeshletMesh>,
    pub meshlet_offset: u32,
    pub meshlet_count: u32,
    pub material: ResourceHandle<Material>,
}

impl Instance {
    pub fn new(
        transform: Transform,
        aabb: Aabb,
        meshlet_mesh_handle: ResourceHandle<MeshletMesh>,
        meshlet_offset: u32,
        meshlet_count: u32,
        material_handle: ResourceHandle<Material>,
    ) -> Self {
        Self {
            transform,
            aabb,
            meshlet: meshlet_mesh_handle,
            meshlet_offset,
            meshlet_count,
            material: material_handle,
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
