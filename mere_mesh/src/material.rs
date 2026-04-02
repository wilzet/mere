use wgpu::util::DeviceExt;

use crate::Texture;

#[derive(Clone, Debug)]
pub struct Material {
    pub name: String,
    pub base_color: [f32; 4],
    pub buffer: wgpu::Buffer,
    pub diffuse_texture: Texture,
    pub normal_texture: Texture,
    pub roughness_metalness_texture: Texture,
    pub bind_group: wgpu::BindGroup,
}

impl Material {
    pub fn new(
        name: &str,
        color: [f32; 4],
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let color_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{name}_albedo")),
            contents: bytemuck::cast_slice(&color),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let diffuse_texture = Texture::create_1x1_texture(device, queue, [0xff; 4], None);
        let roughness_metalness_texture = Texture::create_1x1_texture(device, queue, [0; 4], None);
        let normal_texture = Texture::create_1x1_texture(device, queue, [0; 4], None);

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout,
            entries: &[
                diffuse_texture.bind_group_entry_view(0),
                diffuse_texture.bind_group_entry_sampler(1),
                normal_texture.bind_group_entry_view(2),
                normal_texture.bind_group_entry_sampler(3),
                roughness_metalness_texture.bind_group_entry_view(4),
                roughness_metalness_texture.bind_group_entry_sampler(5),
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
            diffuse_texture,
            normal_texture,
            roughness_metalness_texture,
            bind_group,
        }
    }

    pub fn from_gltf_material(
        name: &str,
        material: gltf::Material,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        textures: &Vec<Texture>,
    ) -> Self {
        let base_color = material.pbr_metallic_roughness().base_color_factor();
        let color_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{name}_albedo")),
            contents: bytemuck::cast_slice(&base_color),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let diffuse_texture = material
            .pbr_metallic_roughness()
            .base_color_texture()
            .map_or_else(
                || Texture::create_1x1_texture(device, queue, [0xff; 4], None),
                |tex| textures[tex.texture().index()].clone(),
            );
        let roughness_metalness_texture = material
            .pbr_metallic_roughness()
            .metallic_roughness_texture()
            .map_or_else(
                || Texture::create_1x1_texture(device, queue, [0; 4], None),
                |tex| textures[tex.texture().index()].clone(),
            );
        let normal_texture = material.normal_texture().map_or_else(
            || Texture::create_1x1_texture(device, queue, [0; 4], None),
            |tex| textures[tex.texture().index()].clone(),
        );

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout,
            entries: &[
                diffuse_texture.bind_group_entry_view(0),
                diffuse_texture.bind_group_entry_sampler(1),
                normal_texture.bind_group_entry_view(2),
                normal_texture.bind_group_entry_sampler(3),
                roughness_metalness_texture.bind_group_entry_view(4),
                roughness_metalness_texture.bind_group_entry_sampler(5),
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: color_buffer.as_entire_binding(),
                },
            ],
        });

        Self {
            name: name.to_string(),
            base_color,
            buffer: color_buffer,
            diffuse_texture,
            normal_texture,
            roughness_metalness_texture,
            bind_group,
        }
    }
}
