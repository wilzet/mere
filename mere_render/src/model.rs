use mere_asset::Mesh;

pub trait DrawModel<'a> {
    fn draw_mesh(
        &mut self,
        mesh: DrawItem,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    );

    fn draw_meshes(
        &mut self,
        meshes: Vec<DrawItem>,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    );
}

pub trait DrawLight<'a> {
    fn draw_light_mesh(
        &mut self,
        mesh: DrawItem,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    );

    #[allow(unused)]
    fn draw_light_meshes(
        &mut self,
        meshes: Vec<DrawItem>,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    );
}

impl<'a, 'b> DrawModel<'b> for wgpu::RenderPass<'a>
where
    'b: 'a,
{
    fn draw_mesh(
        &mut self,
        DrawItem {
            instance_index,
            ref mesh,
            ref material,
        }: DrawItem,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) {
        self.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        self.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        self.set_bind_group(0, camera_bind_group, &[]);
        self.set_bind_group(1, light_bind_group, &[]);
        self.set_bind_group(2, material, &[]);
        self.draw_indexed(0..mesh.index_count, 0, instance_index..instance_index + 1);
    }

    fn draw_meshes(
        &mut self,
        meshes: Vec<DrawItem>,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) {
        for mesh in meshes {
            self.draw_mesh(mesh, camera_bind_group, light_bind_group);
        }
    }
}

impl<'a, 'b> DrawLight<'b> for wgpu::RenderPass<'a> {
    fn draw_light_mesh(
        &mut self,
        DrawItem {
            instance_index,
            ref mesh,
            ..
        }: DrawItem,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) {
        self.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        self.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        self.set_bind_group(0, camera_bind_group, &[]);
        self.set_bind_group(1, light_bind_group, &[]);
        self.draw_indexed(0..mesh.index_count, 0, instance_index..instance_index + 1);
    }

    fn draw_light_meshes(
        &mut self,
        meshes: Vec<DrawItem>,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'b wgpu::BindGroup,
    ) {
        for mesh in meshes {
            self.draw_light_mesh(mesh, camera_bind_group, light_bind_group);
        }
    }
}

#[derive(Clone, Debug)]
pub struct DrawItem {
    pub instance_index: u32,
    pub mesh: Mesh,
    pub material: wgpu::BindGroup,
}
