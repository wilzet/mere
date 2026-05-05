use crate::{
    asset_server::{AssetServer, DefaultResource, Resource},
    handle::ResourceHandle,
    texture::Texture,
};
use gltf::Texture as GltfTexture;
use std::sync::OnceLock;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
pub struct MaterialProperties {
    base_color: [f32; 4],
    normal_scale: f32,
    roughness: f32,
    metalness: f32,
    _pad: [f32; 1],
}

#[derive(Clone, Debug)]
pub struct Material {
    pub name: String,
    pub properties: MaterialProperties,
    pub buffer: wgpu::Buffer,
    pub diffuse: ResourceHandle<Texture>,
    pub normal: ResourceHandle<Texture>,
    pub rough_metal: ResourceHandle<Texture>,
    pub alpha_blended: bool,
    pub bind_group: Option<wgpu::BindGroup>,
}

static MATERIAL_BIND_GROUP: OnceLock<wgpu::BindGroupLayout> = OnceLock::new();

impl Material {
    /// Handle for the default material.
    pub const DEFAULT_MATERIAL_ID: ResourceHandle<Self> = ResourceHandle::new(0);
    pub(crate) const DEFAULT_MATERIAL_NAME: &str = "mere_default_material";

    const DEFAULT_ROUGHNESS: f32 = 0.5;

    pub(crate) fn default_material(device: &wgpu::Device, asset_server: &AssetServer) -> Self {
        let mut material = Self::new(Self::DEFAULT_MATERIAL_NAME, [1.0, 0.0, 1.0, 1.0], device);
        material.try_finish(device, asset_server);
        material
    }

    pub(crate) fn new(name: &str, color: [f32; 4], device: &wgpu::Device) -> Self {
        let properties = MaterialProperties {
            base_color: color,
            normal_scale: 0.0,
            roughness: Self::DEFAULT_ROUGHNESS,
            metalness: 0.0,
            _pad: [0.0; 1],
        };

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{name}_properties")),
            contents: bytemuck::cast_slice(&[properties]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let alpha_blended = color[3] < 1.0;

        Self {
            name: name.to_string(),
            properties,
            buffer,
            diffuse: Texture::DEFAULT_CHEQUERED_TEXTURE_ID,
            normal: Texture::DEFAULT_WHITE_TEXTURE_ID,
            rough_metal: Texture::DEFAULT_WHITE_TEXTURE_ID,
            alpha_blended,
            bind_group: None,
        }
    }

    pub(crate) fn from_gltf_material(
        name: &str,
        material: gltf::Material,
        device: &wgpu::Device,
        asset_server: &AssetServer,
    ) -> Self {
        let map_handle = |tex: GltfTexture<'_>| {
            let handle = ResourceHandle::from(tex.name().unwrap());
            asset_server.reserve_handle(handle);
            handle
        };
        let map_error = |name: &str, ty: &str, default: ResourceHandle<Texture>| {
            let local_name = name.to_string();
            let local_ty = ty.to_string();
            move || {
                mere_log::warn!("material {local_name} is missing texture {local_ty}");
                default
            }
        };

        let diffuse_texture_handle = material
            .pbr_metallic_roughness()
            .base_color_texture()
            .map_or_else(
                map_error(name, "diffuse", Texture::DEFAULT_WHITE_TEXTURE_ID),
                |info| map_handle(info.texture()),
            );
        let (roughness, metalness, rough_metal_texture_handle) = material
            .pbr_metallic_roughness()
            .metallic_roughness_texture()
            .map_or_else(
                || {
                    (
                        Self::DEFAULT_ROUGHNESS,
                        0.0,
                        map_error(name, "rough_metal", Texture::DEFAULT_WHITE_TEXTURE_ID)(),
                    )
                },
                |info| (1.0, 1.0, map_handle(info.texture())),
            );
        let (normal_scale, normal_texture_handle) = material.normal_texture().map_or_else(
            || {
                (
                    0.0,
                    map_error(name, "normal", Texture::DEFAULT_WHITE_TEXTURE_ID)(),
                )
            },
            |normal| (normal.scale(), map_handle(normal.texture())),
        );

        let base_color = material.pbr_metallic_roughness().base_color_factor();

        let properties = MaterialProperties {
            base_color,
            normal_scale,
            roughness,
            metalness,
            _pad: [0.0; 1],
        };

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{name}_properties")),
            contents: bytemuck::cast_slice(&[properties]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let alpha_blended =
            base_color[3] < 1.0 || material.alpha_mode() == gltf::material::AlphaMode::Blend;

        Self {
            name: name.to_string(),
            properties,
            buffer,
            diffuse: diffuse_texture_handle,
            normal: normal_texture_handle,
            rough_metal: rough_metal_texture_handle,
            alpha_blended,
            bind_group: None,
        }
    }

    pub(crate) fn try_finish(&mut self, device: &wgpu::Device, asset_server: &AssetServer) {
        let diffuse_texture =
            asset_server.get_with_default(self.diffuse, Texture::DEFAULT_WHITE_TEXTURE_ID);
        let diffuse_texture = diffuse_texture.read();

        let rough_metal_texture =
            asset_server.get_with_default(self.rough_metal, Texture::DEFAULT_WHITE_TEXTURE_ID);
        let rough_metal_texture = rough_metal_texture.read();

        let normal_texture =
            asset_server.get_with_default(self.normal, Texture::DEFAULT_WHITE_TEXTURE_ID);
        let normal_texture = normal_texture.read();

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: Self::material_bind_group_layout(device),
            entries: &[
                diffuse_texture.bind_group_entry_view(0),
                diffuse_texture.bind_group_entry_sampler(1),
                normal_texture.bind_group_entry_view(2),
                normal_texture.bind_group_entry_sampler(3),
                rough_metal_texture.bind_group_entry_view(4),
                rough_metal_texture.bind_group_entry_sampler(5),
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: self.buffer.as_entire_binding(),
                },
            ],
        });

        self.bind_group = Some(bind_group);
    }

    /// Returns the shared material bind group layout.
    pub fn material_bind_group_layout(device: &wgpu::Device) -> &wgpu::BindGroupLayout {
        MATERIAL_BIND_GROUP.get_or_init(|| device.create_bind_group_layout(&Self::desc()))
    }

    const fn desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            label: Some("material_bind_group_layout"),
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
        }
    }
}
