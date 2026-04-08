use std::{any::TypeId, hash::Hash, marker::PhantomData, ops::Deref};

type ResourceHandleID = u64;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct UntypedHandle(ResourceHandle<TypeId>);

impl Deref for UntypedHandle {
    type Target = ResourceHandleID;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<R: 'static> From<ResourceHandle<R>> for UntypedHandle {
    fn from(value: ResourceHandle<R>) -> Self {
        Self(ResourceHandle::new(value.id))
    }
}

#[derive(Debug, Default)]
pub struct ResourceHandle<R> {
    id: ResourceHandleID,
    _type: PhantomData<R>,
}

impl<R> ResourceHandle<R> {
    pub const fn new(id: ResourceHandleID) -> Self {
        Self {
            id,
            _type: PhantomData,
        }
    }
}

impl<R> Clone for ResourceHandle<R> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            _type: PhantomData,
        }
    }
}

impl<R> Copy for ResourceHandle<R> {}

impl<R> PartialEq for ResourceHandle<R> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<R> Eq for ResourceHandle<R> {}

impl<R> Hash for ResourceHandle<R> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl<R> Deref for ResourceHandle<R> {
    type Target = ResourceHandleID;

    fn deref(&self) -> &Self::Target {
        &self.id
    }
}

impl<R> From<&[u8]> for ResourceHandle<R> {
    fn from(value: &[u8]) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(value);

        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&hasher.finalize().as_bytes()[..8]);
        Self::new(ResourceHandleID::from_le_bytes(bytes))
    }
}

impl<R> From<usize> for ResourceHandle<R> {
    fn from(value: usize) -> Self {
        Self::from(value.to_le_bytes().as_slice())
    }
}

impl<R> From<&str> for ResourceHandle<R> {
    fn from(value: &str) -> Self {
        Self::from(value.as_bytes())
    }
}
