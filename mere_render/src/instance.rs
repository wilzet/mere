use mere_math::Transform;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Debug)]
pub(crate) struct InstanceRaw {
    model: [[f32; 4]; 4],
    normal: [[f32; 3]; 3],
}

impl InstanceRaw {
    pub const ATTRIBUTES: [wgpu::VertexAttribute; 7] = wgpu::vertex_attr_array![5 => Float32x4, 6 => Float32x4, 7 => Float32x4, 8 => Float32x4, 9 => Float32x3, 10 => Float32x3, 11 => Float32x3];

    pub const fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<InstanceRaw>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBUTES,
        }
    }

    pub fn from_transform(transform: Transform) -> Self {
        let normal = transform.rotation_matrix().to_cols_array_2d();
        let model = transform.trs().to_cols_array_2d();

        Self { model, normal }
    }
}
