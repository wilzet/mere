use glam::{Mat4, Quat, Vec3};

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Transform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Transform {
    pub const IDENTITY: Self = Self {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    };

    #[inline]
    pub const fn from_translation(translation: Vec3) -> Self {
        Self {
            translation,
            ..Self::IDENTITY
        }
    }

    #[inline]
    pub const fn from_rotation(rotation: Quat) -> Self {
        Self {
            rotation,
            ..Self::IDENTITY
        }
    }

    #[inline]
    pub const fn from_scale(scale: Vec3) -> Self {
        Self {
            scale,
            ..Self::IDENTITY
        }
    }

    #[inline]
    pub fn inverse(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation).inverse()
    }

    #[inline]
    pub fn forward(&self) -> Vec3 {
        self.rotation * Vec3::NEG_Z
    }

    #[inline]
    pub fn up(&self) -> Vec3 {
        self.rotation * Vec3::Y
    }

    #[inline]
    pub fn right(&self) -> Vec3 {
        self.rotation * Vec3::X
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}
