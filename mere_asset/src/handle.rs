use std::ops::Deref;

pub(crate) type ResourceHandleID = u64;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct ResourceHandle(ResourceHandleID);

impl Deref for ResourceHandle {
    type Target = ResourceHandleID;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<ResourceHandleID> for ResourceHandle {
    fn from(value: ResourceHandleID) -> Self {
        Self(value)
    }
}

impl ResourceHandle {
    pub(crate) fn from_name(name: &str) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(name.as_bytes());

        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&hasher.finalize().as_bytes()[..8]);
        Self(ResourceHandleID::from_le_bytes(bytes))
    }
}
