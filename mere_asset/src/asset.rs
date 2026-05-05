use crate::{
    asset_server::{AssetEvent, AssetServer},
    gpu_buffer::GpuBufferable,
    handle::{ResourceHandle, UntypedHandle},
    material::Material,
    texture::{Texture, TextureOptions},
};
use anyhow::Context;
use gltf::Material as GltfMaterial;
use image::GenericImageView;
use mere_asset_common::collect_gltf_files;
use mere_common::{ASSET_DIR, PROCESSED_ASSET_DIR};
use mere_mesh::MeshletMesh;
use std::path::{Path, PathBuf};

pub trait Asset: Sized {
    type Source<'a>;

    fn load(source: Self::Source<'_>) -> anyhow::Result<Self>;

    fn dependencies(&self) -> Vec<UntypedHandle> {
        Vec::with_capacity(0)
    }

    fn finish(&mut self, _asset_server: &AssetServer) {}

    fn size_in_bytes(&self) -> usize;
}

impl Asset for MeshletMesh {
    type Source<'a> = (&'a str, usize, mere_mesh::MeshletMesh);

    fn load(source: Self::Source<'_>) -> anyhow::Result<Self> {
        let (name, index, mut mesh) = source;
        let name = format!("{name}_{index}");

        mesh.set_name(name);

        Ok(mesh)
    }

    fn finish(&mut self, asset_server: &AssetServer) {
        let _ = asset_server.send(AssetEvent::MeshletReady(ResourceHandle::from(
            self.name.as_str(),
        )));
    }

    fn size_in_bytes(&self) -> usize {
        let mut total = size_of::<Self>();

        total += self.name.len();
        total += self.vertices.size_in_bytes();
        total += self.meshlet_vertex_indices.size_in_bytes();
        total += self.meshlet_indices.size_in_bytes();
        total += self.meshlets.size_in_bytes();
        total
    }
}

impl Asset for Texture {
    type Source<'a> = (
        &'a Path,
        &'a wgpu::Device,
        &'a wgpu::Queue,
        TextureOptions,
        u32,
    );

    fn load(source: Self::Source<'_>) -> anyhow::Result<Self> {
        let (path, device, queue, options, mip_0) = source;

        let label = match path.file_name().and_then(|s| s.to_str()) {
            Some(name) => name,
            None => anyhow::bail!("No file found in path: {path:?}"),
        };

        let image = {
            let img = image::open(path)?;
            let (width, height) = img.dimensions();
            let factor = 1 << mip_0;
            img.resize(
                (width / factor).max(1),
                (height / factor).max(1),
                image::imageops::FilterType::Triangle,
            )
        };

        Ok(Self::from_image(device, queue, image, label, options)?)
    }

    fn size_in_bytes(&self) -> usize {
        (self.view.texture().size().height * self.view.texture().size().width * 4) as usize
    }
}

impl Asset for Material {
    type Source<'a> = (&'a str, GltfMaterial<'a>, &'a wgpu::Device, &'a AssetServer);

    fn load(source: Self::Source<'_>) -> anyhow::Result<Self> {
        let (path, material, device, asset_server) = source;

        let default_name = format!("{path}_mat_{}", material.index().unwrap_or(0));
        let name = material.name().unwrap_or(&default_name);

        Ok(Self::from_gltf_material(
            name,
            material,
            device,
            asset_server,
        ))
    }

    fn dependencies(&self) -> Vec<UntypedHandle> {
        vec![
            self.diffuse.into(),
            self.rough_metal.into(),
            self.normal.into(),
        ]
    }

    fn finish(&mut self, asset_server: &AssetServer) {
        self.try_finish(asset_server.device(), asset_server);
    }

    fn size_in_bytes(&self) -> usize {
        let mut total = size_of::<Self>();

        total += self.name.len();
        total
    }
}

pub fn load_mere_meshes(path: impl AsRef<Path>) -> anyhow::Result<Vec<mere_mesh::MeshletMesh>> {
    let model_path = PathBuf::from(PROCESSED_ASSET_DIR)
        .join(path)
        .with_extension("mere");

    mere_mesh::read_mere_file(&model_path)
}

pub struct GltfAsset {
    document: gltf::Document,
}

impl GltfAsset {
    pub fn nodes(&self) -> gltf::iter::Nodes<'_> {
        self.document.nodes()
    }

    pub fn models(&self) -> gltf::iter::Meshes<'_> {
        self.document.meshes()
    }

    pub fn images(&self) -> Vec<gltf::Image<'_>> {
        self.document.images().collect()
    }

    pub fn materials(&self) -> gltf::iter::Materials<'_> {
        self.document.materials()
    }
}

pub fn load_gltf_asset(path: impl AsRef<Path>) -> anyhow::Result<GltfAsset> {
    let gltf_path = PathBuf::from(ASSET_DIR).join(path);
    let gltf_paths = collect_gltf_files(&gltf_path)
        .context(format!("No `.gltf` file found in {gltf_path:?}"))?;

    if gltf_paths.len() != 1 {
        anyhow::bail!("Expected exactly one `.gltf` file in {gltf_path:?}");
    }

    let gltf_path = &gltf_paths[0];

    Ok(GltfAsset {
        document: gltf::Gltf::open(gltf_path)?.document,
    })
}
