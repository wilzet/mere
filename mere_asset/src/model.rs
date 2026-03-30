use mere_math::Transform;
use std::rc::Rc;

pub(crate) type ModelHandleID = u64;

#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct ModelHandle {
    pub(crate) id: ModelHandleID,
    pub(crate) _ref_counter: Rc<()>,
}

impl ModelHandle {
    pub(crate) fn id_from_path(path: &str) -> ModelHandleID {
        let mut hasher = blake3::Hasher::new();
        hasher.update(path.as_bytes());

        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&hasher.finalize().as_bytes()[..8]);
        ModelHandleID::from_le_bytes(bytes)
    }

    pub(crate) fn use_count(&self) -> usize {
        Rc::strong_count(&self._ref_counter)
    }

    pub fn id(&self) -> ModelHandleID {
        self.id
    }
}

#[derive(Clone, Debug, Default)]
pub struct ModelInstance {
    pub(crate) handle: ModelHandle,
    pub(crate) transform: Transform,
}

impl ModelInstance {
    pub fn handle(&self) -> ModelHandleID {
        self.handle.id
    }
}
