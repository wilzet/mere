use crate::{
    camera::Camera,
    texture::{Texture, TextureOptions},
};
use mere_math::Vec4Swizzles;

#[derive(Clone, Debug)]
pub struct MeshletBindGroups {
    pub instance_cull_bind_group: wgpu::BindGroup,
    pub cluster_cull_bind_group: wgpu::BindGroup,
    pub meshlet_mesh_material_bind_group: wgpu::BindGroup,
    pub main_render_view_bind_group: wgpu::BindGroup,
    pub render_view_bind_group: wgpu::BindGroup,
}

#[derive(Clone, Debug)]
pub struct PerFrameResources {
    pub indirect_cluster_args: wgpu::Buffer,
    pub indirect_draw_args: wgpu::Buffer,
    pub bind_groups: MeshletBindGroups,
}

#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Debug)]
#[repr(C)]
pub struct RenderView {
    view_position: [f32; 4],
    view_proj: [[f32; 4]; 4],
    frustum: [[f32; 4]; 6],
}

impl RenderView {
    pub fn from_camera(camera: &Camera) -> Self {
        let view_proj = camera.projection_matrix() * camera.view_matrix();

        let row = |i: usize| view_proj.row(i);

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
            view_position: camera.transform.translation.to_homogeneous().into(),
            view_proj: view_proj.to_cols_array_2d(),
            frustum: planes.map(|p| p.to_array()),
        }
    }
}

const DEPTH_PYRAMID_COUNT: usize = 12;

#[derive(Clone, Debug)]
pub struct DepthPyramid {
    pub depth: Texture,
    pub depth_pyramid: Texture,
    pub depth_pyramid_mips: [Option<wgpu::TextureView>; DEPTH_PYRAMID_COUNT],
    pub mip_count: u32,
}

impl DepthPyramid {
    pub fn new(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        label: &str,
        width: u32,
        height: u32,
    ) -> Self {
        let size = wgpu::Extent3d {
            width: width.next_power_of_two() / 2,
            height: height.next_power_of_two() / 2,
            depth_or_array_layers: 1,
        };

        let mip_count = size.max_mips(wgpu::TextureDimension::D2);

        let depth_pyramid = Texture::create_texture(
            label,
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size,
                mip_level_count: mip_count,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R32Float,
                usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            }),
            TextureOptions::default(),
            device,
        );

        let depth_pyramid_mips = std::array::from_fn(|i| {
            if (i as u32) < mip_count {
                Some(
                    depth_pyramid
                        .texture()
                        .create_view(&wgpu::TextureViewDescriptor {
                            label: Some(label),
                            format: Some(wgpu::TextureFormat::R32Float),
                            dimension: Some(wgpu::TextureViewDimension::D2),
                            usage: None,
                            aspect: wgpu::TextureAspect::All,
                            base_mip_level: i as u32,
                            mip_level_count: Some(1),
                            base_array_layer: 0,
                            array_layer_count: Some(1),
                        }),
                )
            } else {
                None
            }
        });

        Self {
            depth: Texture::create_depth_texture(&device, &config, "depth_texture"),
            depth_pyramid,
            depth_pyramid_mips,
            mip_count,
        }
    }
}
