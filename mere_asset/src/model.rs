use crate::{handle::ResourceHandle, material::Material};
use mere_mesh::MereMesh;
use wgpu::util::DeviceExt;

#[derive(Clone, Debug)]
pub struct Model {
    pub name: String,
    pub meshes: Vec<Mesh>,
}

impl Model {
    pub fn new(name: &str, meshes: Vec<Mesh>) -> Self {
        Self {
            name: name.to_string(),
            meshes: meshes,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn meshes(&self) -> impl Iterator<Item = &Mesh> {
        self.meshes.iter()
    }
}

#[derive(Clone, Debug)]
pub struct Mesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_elements: u32,
    pub material: ResourceHandle<Material>,
}

impl Mesh {
    pub fn from_mere_mesh(name: &str, mesh: MereMesh, device: &wgpu::Device) -> Self {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{}_vertex_buffer", name)),
            contents: bytemuck::cast_slice(&mesh.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{}_index_buffer", name)),
            contents: bytemuck::cast_slice(&mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Mesh {
            vertex_buffer,
            index_buffer,
            num_elements: mesh.indices.len() as u32,
            material: ResourceHandle::from(Material::DEFAULT_MATERIAL_NAME),
        }
    }

    pub fn with_material(self, material_handle: ResourceHandle<Material>) -> Self {
        Self {
            material: material_handle,
            ..self
        }
    }
}
