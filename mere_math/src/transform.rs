use glam::{Affine3, EulerRot, Mat3, Mat4, Quat, Vec3};
use std::ops::Mul;

/// TRS transform in 3D space (translation, rotation, scale).
/// Stored in engine-native form and convertible to [`Mat4`].
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

    pub const fn new() -> Self {
        Self::IDENTITY
    }

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
    pub const fn with_translation(self, translation: Vec3) -> Self {
        Self {
            translation,
            ..self
        }
    }

    #[inline]
    pub const fn with_rotation(self, rotation: Quat) -> Self {
        Self { rotation, ..self }
    }

    #[inline]
    pub const fn with_scale(self, scale: Vec3) -> Self {
        Self { scale, ..self }
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

    #[inline]
    pub fn euler_angles(&self) -> Vec3 {
        self.rotation.to_euler(EulerRot::XYZ).into()
    }

    #[inline]
    pub fn world_from_local_transpose(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
            .transpose()
    }

    #[inline]
    pub fn local_from_world(&self) -> Mat3 {
        Affine3::from_mat4(self.inverse()).matrix3.transpose()
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Mul for Transform {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self {
            translation: self.translation + (self.rotation * (self.scale * rhs.translation)),
            rotation: self.rotation * rhs.rotation,
            scale: self.scale * rhs.scale,
        }
    }
}
