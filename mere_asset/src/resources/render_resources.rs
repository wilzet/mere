use crate::{
    camera::Camera,
};
use mere_math::Vec4Swizzles;

#[derive(Clone, Debug)]
pub struct MeshletBindGroups {
    pub visibility_buffer_clear_bind_group: wgpu::BindGroup,
    pub instance_cull_bind_group: wgpu::BindGroup,
    pub cluster_cull_bind_group: wgpu::BindGroup,
    pub visibility_buffer_raster_bind_group: wgpu::BindGroup,
    pub meshlet_read_attributes_bind_group: wgpu::BindGroup,
    pub main_render_view_bind_group: wgpu::BindGroup,
    pub render_view_bind_group: wgpu::BindGroup,
    pub downsample_depth_bind_group: wgpu::BindGroup,
    pub resolve_material_depth_bind_group: wgpu::BindGroup,
}

#[derive(Clone, Debug)]
pub struct PerFrameResources {
    pub visibility_buffer: wgpu::TextureView,
    pub dummy_render_target: wgpu::TextureView,
    pub indirect_cluster_args: wgpu::Buffer,
    pub indirect_draw_args: wgpu::Buffer,
    pub bind_groups: MeshletBindGroups,
}

#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Debug)]
#[repr(C)]
pub struct RenderView {
    world_position: [f32; 4],
    viewport: [f32; 4],
    view_proj: [[f32; 4]; 4],
    frustum_planes: [[f32; 4]; 6],
}

impl RenderView {
    pub fn from_camera(camera: &Camera) -> Self {
        let view_proj = camera.projection_matrix() * camera.view_matrix();
        let frustum_matrix = camera.frustum_matrix();

        let row = |i: usize| frustum_matrix.row(i);

        let mut planes = [
            (row(3) + row(0)), // Left
            (row(3) - row(0)), // Right
            (row(3) + row(1)), // Bottom
            (row(3) - row(1)), // Top
            (row(3) + row(2)), // Near
            (row(3) - row(2)), // Far
        ];

        for plane in planes.iter_mut() {
            let length = plane.xyz().length();
            *plane /= length;
        }

        Self {
            world_position: camera.transform.translation.to_homogeneous().into(),
            viewport: camera.viewport.into(),
            view_proj: view_proj.to_cols_array_2d(),
            frustum_planes: planes.map(|p| p.to_array()),
        }
    }
}

pub struct Layout<'a> {
    label: Option<&'a str>,
    entries: Vec<wgpu::BindGroupLayoutEntry>,
}

impl<'a> Layout<'a> {
    pub fn sequential(
        label: Option<&'a str>,
        visibility: wgpu::ShaderStages,
        entries: &[wgpu::BindGroupLayoutEntry],
    ) -> Self {
        let entries = entries
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let mut entry = binding_entry(i as u32, e.ty);
                entry.visibility = visibility;
                entry
            })
            .collect();

        Self { label, entries }
    }

    pub fn with(&mut self, mut entry: wgpu::BindGroupLayoutEntry) -> &mut Self {
        entry.binding = self.entries.len() as u32;
        self.entries.push(entry);
        self
    }

    pub fn get(&self) -> wgpu::BindGroupLayoutDescriptor<'_> {
        wgpu::BindGroupLayoutDescriptor {
            label: self.label,
            entries: &self.entries,
        }
    }
}

pub const fn entry(ty: wgpu::BindingType) -> wgpu::BindGroupLayoutEntry {
    binding_entry(0, ty)
}

pub const fn storage_buffer(read_only: bool) -> wgpu::BindGroupLayoutEntry {
    storage_buffer_binding_entry(0, read_only)
}

pub const fn storage_texture(
    format: wgpu::TextureFormat,
    access: wgpu::StorageTextureAccess,
) -> wgpu::BindGroupLayoutEntry {
    storage_texture_binding_entry(0, format, access)
}

pub const fn binding_entry(binding: u32, ty: wgpu::BindingType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::empty(),
        ty,
        count: None,
    }
}

pub const fn storage_buffer_binding_entry(
    binding: u32,
    read_only: bool,
) -> wgpu::BindGroupLayoutEntry {
    binding_entry(
        binding,
        wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
    )
}

pub const fn storage_texture_binding_entry(
    binding: u32,
    format: wgpu::TextureFormat,
    access: wgpu::StorageTextureAccess,
) -> wgpu::BindGroupLayoutEntry {
    binding_entry(
        binding,
        wgpu::BindingType::StorageTexture {
            access,
            format,
            view_dimension: wgpu::TextureViewDimension::D2,
        },
    )
}
