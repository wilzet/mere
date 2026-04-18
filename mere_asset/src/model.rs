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
            meshes,
        }
    }

    pub fn meshes(&self) -> std::slice::Iter<'_, Mesh> {
        self.meshes.iter()
    }
}

#[derive(Clone, Debug)]
pub struct Mesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
    pub material: ResourceHandle<Material>,
}

impl Mesh {
    pub(crate) fn from_mere_mesh(name: &str, mesh: MereMesh, device: &wgpu::Device) -> Self {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{name}_vertex_buffer")),
            contents: bytemuck::cast_slice(&mesh.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{name}_index_buffer")),
            contents: bytemuck::cast_slice(&mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Mesh {
            vertex_buffer,
            index_buffer,
            index_count: mesh.indices.len() as u32,
            material: ResourceHandle::from(Material::DEFAULT_MATERIAL_NAME),
        }
    }

    pub(crate) fn with_material(self, material_handle: ResourceHandle<Material>) -> Self {
        Self {
            material: material_handle,
            ..self
        }
    }
}
