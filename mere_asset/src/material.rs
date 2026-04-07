use crate::{
    asset::{AssetServer, Resource},
    handle::ResourceHandle,
    texture::Texture,
};
use std::sync::Arc;
use wgpu::util::DeviceExt;

#[derive(Clone, Debug)]
pub struct Material {
    pub name: String,
    pub base_color: [f32; 4],
    pub buffer: wgpu::Buffer,
    pub diffuse_texture: ResourceHandle<Texture>,
    pub normal_texture: ResourceHandle<Texture>,
    pub rough_metal_texture: ResourceHandle<Texture>,
    pub bind_group: wgpu::BindGroup,
}

impl Material {
    pub(crate) fn new(
        name: &str,
        color: [f32; 4],
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        asset_server: &AssetServer,
    ) -> Self {
        let color_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{name}_albedo")),
            contents: bytemuck::cast_slice(&color),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let diffuse_texture = asset_server.get(Texture::DEFAULT_WHITE_TEXTURE_ID).unwrap();
        let rough_metal_texture = asset_server.get(Texture::DEFAULT_BLACK_TEXTURE_ID).unwrap();
        let normal_texture = asset_server.get(Texture::DEFAULT_BLACK_TEXTURE_ID).unwrap();

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout,
            entries: &[
                diffuse_texture.bind_group_entry_view(0),
                diffuse_texture.bind_group_entry_sampler(1),
                normal_texture.bind_group_entry_view(2),
                normal_texture.bind_group_entry_sampler(3),
                rough_metal_texture.bind_group_entry_view(4),
                rough_metal_texture.bind_group_entry_sampler(5),
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: color_buffer.as_entire_binding(),
                },
            ],
        });

        Self {
            name: name.to_string(),
            base_color: color,
            buffer: color_buffer,
            diffuse_texture: Texture::DEFAULT_WHITE_TEXTURE_ID,
            normal_texture: Texture::DEFAULT_BLACK_TEXTURE_ID,
            rough_metal_texture: Texture::DEFAULT_BLACK_TEXTURE_ID,
            bind_group,
        }
    }

    pub(crate) fn from_gltf_material(
        name: &str,
        material: gltf::Material,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        asset_server: &AssetServer,
    ) -> anyhow::Result<Self> {
        let base_color = material.pbr_metallic_roughness().base_color_factor();
        let color_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{name}_albedo")),
            contents: bytemuck::cast_slice(&base_color),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let diffuse_texture_handle = material
            .pbr_metallic_roughness()
            .base_color_texture()
            .map_or(Texture::DEFAULT_WHITE_TEXTURE_ID, |tex| {
                ResourceHandle::from(tex.texture().name().unwrap())
            });
        let rough_metal_texture_handle = material
            .pbr_metallic_roughness()
            .metallic_roughness_texture()
            .map_or(Texture::DEFAULT_BLACK_TEXTURE_ID, |tex| {
                ResourceHandle::from(tex.texture().name().unwrap())
            });
        let normal_texture_handle = material
            .normal_texture()
            .map_or(Texture::DEFAULT_BLACK_TEXTURE_ID, |tex| {
                ResourceHandle::from(tex.texture().name().unwrap())
            });

        let diffuse_texture =
            get_texture_with_error(asset_server, diffuse_texture_handle, name, "diffuse")?;
        let rough_metal_texture = get_texture_with_error(
            asset_server,
            rough_metal_texture_handle,
            name,
            "roughness/metalness",
        )?;
        let normal_texture =
            get_texture_with_error(asset_server, normal_texture_handle, name, "normal")?;

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout,
            entries: &[
                diffuse_texture.bind_group_entry_view(0),
                diffuse_texture.bind_group_entry_sampler(1),
                normal_texture.bind_group_entry_view(2),
                normal_texture.bind_group_entry_sampler(3),
                rough_metal_texture.bind_group_entry_view(4),
                rough_metal_texture.bind_group_entry_sampler(5),
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: color_buffer.as_entire_binding(),
                },
            ],
        });

        Ok(Self {
            name: name.to_string(),
            base_color,
            buffer: color_buffer,
            diffuse_texture: diffuse_texture_handle,
            normal_texture: normal_texture_handle,
            rough_metal_texture: rough_metal_texture_handle,
            bind_group,
        })
    }

    pub const fn desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
            label: Some("material_bind_group_layout"),
        }
    }
}

fn get_texture_with_error(
    asset_server: &AssetServer,
    handle: ResourceHandle<Texture>,
    name: &str,
    ty: &str,
) -> anyhow::Result<Arc<Texture>> {
    match asset_server.get(handle) {
        Some(tex) => Ok(tex),
        None => anyhow::bail!("material {name} is missing texture {ty}"),
    }
}
