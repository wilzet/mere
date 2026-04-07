use crate::material::Material;
use mere_mesh::Mesh;

#[derive(Clone, Debug)]
pub struct Model {
    pub name: String,
    pub meshes: Vec<Mesh>,
    pub materials: Vec<Material>,
}

impl Model {
    pub fn new(name: &str, meshes: Vec<Mesh>, materials: Vec<Material>) -> Self {
        Self {
            name: name.to_string(),
            meshes,
            materials,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn meshes(&self) -> impl Iterator<Item = &Mesh> {
        self.meshes.iter()
    }
}
