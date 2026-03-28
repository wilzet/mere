use mere_math::Transform;
use mere_mesh::Mesh;
use std::ops::Deref;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct MeshHandle(u64);

impl Deref for MeshHandle {
    type Target = u64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl MeshHandle {
    pub(crate) fn from_mesh(mesh: &Mesh) -> Self {
        let mut hasher = blake3::Hasher::new();

        for &idx in &mesh.indices {
            hasher.update(&idx.to_le_bytes());
        }

        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&hasher.finalize().as_bytes()[..8]);

        Self(u64::from_le_bytes(bytes))
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MeshInstance {
    pub(crate) handle: MeshHandle,
    pub(crate) transform: Transform,
}
