use crate::handle::{ResourceHandle, ResourceHandleID};
use mere_math::Transform;

#[derive(Clone, Copy, Debug, Default)]
pub struct ModelInstance {
    pub(crate) handle: ResourceHandle,
    pub(crate) transform: Transform,
}

impl ModelInstance {
    pub fn handle(&self) -> ResourceHandleID {
        *self.handle
    }
}
