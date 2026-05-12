use crate::{
    gpu_buffer::{GpuBufferable, GpuStorageBuffer},
    handle::ResourceHandle,
    material::Material,
};
use mere_math::Transform;
use mere_mesh::{Aabb, MeshletMesh};
use slotmap::DenseSlotMap;
use std::collections::HashSet;

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
    pub materials_in_scene: HashSet<ResourceHandle<Material>>,
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
            materials_in_scene: HashSet::new(),
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
        self.materials_in_scene.insert(instance.material);
        self.instances.insert(instance)
    }

    pub fn count_clusters(&self) -> usize {
        self.instances
            .iter()
            .fold(0, |acc, (_, i)| i.meshlet_count + acc) as usize
    }

    pub fn build_instance_buffers(&mut self) {
        self.reset();

        for (_, instance) in self.instances.iter() {
            let mesh_uniform = MeshUniform::new(instance.transform, instance.previous_transform);

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
                .push(instance.material_id);
        }

        self.scene_instance_count = self.instances.len() as u32;
    }

    fn reset(&mut self) {
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
    pub previous_transform: Transform,
    pub aabb: Aabb,
    pub meshlet_mesh: ResourceHandle<MeshletMesh>,
    pub meshlet_offset: u32,
    pub meshlet_count: u32,
    pub material: ResourceHandle<Material>,
    pub material_id: u32,
}

impl Instance {
    pub fn new(
        transform: Transform,
        aabb: Aabb,
        meshlet_mesh_handle: ResourceHandle<MeshletMesh>,
        meshlet_offset: u32,
        meshlet_count: u32,
        material_handle: ResourceHandle<Material>,
        material_id: u32,
    ) -> Self {
        Self {
            transform,
            previous_transform: transform,
            aabb,
            meshlet_mesh: meshlet_mesh_handle,
            meshlet_offset,
            meshlet_count,
            material: material_handle,
            material_id,
        }
    }
}

#[derive(Clone, Copy, Default, Debug)]
#[repr(C)]
pub struct MeshUniform {
    model: [[f32; 4]; 3],
    previous_model: [[f32; 4]; 3],
    inverse_transpose_a: [[f32; 4]; 2],
    inverse_transpose_b: f32,
    _padding: [f32; 3],
}

unsafe impl bytemuck::Pod for MeshUniform {}
unsafe impl bytemuck::Zeroable for MeshUniform {}

impl MeshUniform {
    fn new(transform: Transform, previous_transform: Transform) -> Self {
        let model = {
            let [col0, col1, col2, _] = transform.world_from_local_transpose().to_cols_array_2d();
            [col0.into(), col1.into(), col2.into()]
        };
        let previous_model = {
            let [col0, col1, col2, _] = previous_transform
                .world_from_local_transpose()
                .to_cols_array_2d();
            [col0.into(), col1.into(), col2.into()]
        };
        let (inverse_transpose_a, inverse_transpose_b) = {
            let local_from_world = transform.local_from_world().to_cols_array_2d();
            (
                [
                    [
                        local_from_world[0][0],
                        local_from_world[0][1],
                        local_from_world[0][2],
                        local_from_world[1][0],
                    ],
                    [
                        local_from_world[1][1],
                        local_from_world[1][2],
                        local_from_world[2][0],
                        local_from_world[2][1],
                    ],
                ],
                local_from_world[2][2],
            )
        };

        Self {
            model,
            previous_model,
            inverse_transpose_a,
            inverse_transpose_b,
            _padding: [0.0, 0.0, 0.0],
        }
    }
}
