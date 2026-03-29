use mere_math::Transform;
use std::rc::Rc;

pub(crate) type MeshHandleID = u64;

#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct MeshHandle {
    pub(crate) id: MeshHandleID,
    pub(crate) _ref_counter: Rc<()>,
}

impl MeshHandle {
    pub(crate) fn id_from_path(path: &str) -> MeshHandleID {
        let mut hasher = blake3::Hasher::new();
        hasher.update(path.as_bytes());

        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&hasher.finalize().as_bytes()[..8]);
        MeshHandleID::from_le_bytes(bytes)
    }

    pub(crate) fn use_count(&self) -> usize {
        Rc::strong_count(&self._ref_counter)
    }
}

#[derive(Clone, Debug, Default)]
pub struct MeshInstance {
    pub(crate) handle: MeshHandle,
    pub(crate) transform: Transform,
}

impl MeshInstance {
    pub fn handle(&self) -> MeshHandleID {
        self.handle.id
    }
}
