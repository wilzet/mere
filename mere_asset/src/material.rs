use crate::{
    asset::{AssetServer, GetResource},
    handle::ResourceHandle,
    texture::Texture,
};
use std::{error::Error, fmt::Display};
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
                ResourceHandle::from(tex.texture().index())
            });
        let rough_metal_texture_handle = material
            .pbr_metallic_roughness()
            .metallic_roughness_texture()
            .map_or(Texture::DEFAULT_BLACK_TEXTURE_ID, |tex| {
                ResourceHandle::from(tex.texture().index())
            });
        let normal_texture_handle = material
            .normal_texture()
            .map_or(Texture::DEFAULT_BLACK_TEXTURE_ID, |tex| {
                ResourceHandle::from(tex.texture().index())
            });

        let diffuse_texture = asset_server.get(diffuse_texture_handle).ok_or_else(|| {
            MaterialError::TextureDoesNotExist {
                name: name.to_string(),
                texture: "diffuse".to_string(),
            }
        })?;
        let rough_metal_texture =
            asset_server
                .get(rough_metal_texture_handle)
                .ok_or_else(|| MaterialError::TextureDoesNotExist {
                    name: name.to_string(),
                    texture: "diffuse".to_string(),
                })?;
        let normal_texture = asset_server.get(normal_texture_handle).ok_or_else(|| {
            MaterialError::TextureDoesNotExist {
                name: name.to_string(),
                texture: "diffuse".to_string(),
            }
        })?;

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
}

#[derive(Debug)]
pub enum MaterialError {
    TextureDoesNotExist { name: String, texture: String },
}

impl Display for MaterialError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MaterialError::TextureDoesNotExist { name, texture } => {
                f.write_fmt(format_args!("material {name} is missing {texture}"))
            }
        }
    }
}

impl Error for MaterialError {}
