use crate::Texture;

#[derive(Clone, Debug)]
pub struct Material {
    pub name: String,
    pub base_color: [f32; 4],
    pub diffuse_texture: Texture,
    pub normal_texture: Texture,
    pub roughness_metalness_texture: Texture,
    pub bind_group: wgpu::BindGroup,
}
