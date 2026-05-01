use crate::{
    asset::{Asset, GltfAsset, load_gltf_asset, load_mere_meshes},
    asset_server::{AssetEvent, AssetServer, DefaultResource, Resource, Shared},
    camera::Camera,
    handle::ResourceHandle,
    instances::{Instance, InstanceHandle, InstanceStorage},
    material::Material,
    meshlets::MeshletStorage,
    resources::{MeshletBindGroups, ResourceStorage},
    texture::{MipmapOptions, Texture, TextureOptions},
};
use mere_common::ASSET_DIR;
use mere_math::{Quat, Transform};
use mere_mesh::MeshletMesh;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::path;

#[derive(Debug)]
pub struct Scene {
    cameras: Vec<Camera>,
    asset_server: AssetServer,
    instances: InstanceStorage,
    meshlets: MeshletStorage,
    resources: ResourceStorage,
}

impl Scene {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, cluster_slots: u32) -> Self {
        Self {
            cameras: Vec::new(),
            asset_server: AssetServer::new(device, queue),
            instances: InstanceStorage::new(),
            meshlets: MeshletStorage::new(device),
            resources: ResourceStorage::new(cluster_slots),
        }
    }

    pub fn load_gltf(
        &mut self,
        path: &str,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> anyhow::Result<Vec<InstanceHandle>> {
        let gltf_asset = load_gltf_asset(path)?;

        let asset_server_inner = self.asset_server.clone();
        let device = device.clone();
        let queue = queue.clone();

        let instance_handles = gltf_asset
            .nodes()
            .filter_map(|node| {
                let model = node.mesh()?;
                let default_name = format!("{path}_model_{}", model.index());
                let name = model.name().map_or(default_name, |s| s.to_string());

                let (translation, rotation, scale) = node.transform().decomposed();
                let transform = Transform {
                    translation: translation.into(),
                    rotation: Quat::from_vec4(rotation.into()),
                    scale: scale.into(),
                };

                Some((model, transform, name))
            })
            .flat_map(|(model, transform, name)| {
                model.primitives().map(move |mesh| {
                    let meshlet_mesh_handle =
                        ResourceHandle::from(format!("{name}_{}", mesh.index()).as_str());

                    let material_handle = match mesh.material().name() {
                        Some(name) => ResourceHandle::from(name),
                        None => Material::DEFAULT_MATERIAL_ID,
                    };

                    (transform, meshlet_mesh_handle, material_handle)
                })
            })
            .map(|(transform, meshlet_mesh_handle, material_handle)| {
                asset_server_inner.reserve_handle(meshlet_mesh_handle);
                asset_server_inner.reserve_handle(material_handle);
                self.instances.add_instance(
                    Instance::new(transform, meshlet_mesh_handle),
                    material_handle,
                )
            })
            .collect();

        let path_string = path.to_string();
        std::thread::spawn(move || {
            if let Err(err) =
                background_load_task(&path_string, gltf_asset, device, queue, asset_server_inner)
            {
                mere_log::error!("Async load failed for {path_string}: {err}");
            }
        });

        mere_log::info!("Started background load for {path}");
        Ok(instance_handles)
    }

    pub fn process_asset_event(&mut self) {
        while let Ok(event) = self.asset_server.try_recv() {
            match event {
                AssetEvent::Ready(handle) => self.asset_server.dispatch_ready(handle),
                AssetEvent::MeshletReady(handle) => {
                    let meshlet_mesh_lock = self.asset_server.get(handle).unwrap();
                    let meshlet_mesh = meshlet_mesh_lock.read();
                    self.meshlets.queue_upload(&meshlet_mesh);
                }
            }
        }
    }

    pub fn instances(&self) -> slotmap::dense::Values<'_, InstanceHandle, Instance> {
        self.instances.iter()
    }

    pub fn add_camera(&mut self, camera: Camera) -> usize {
        let id = self.cameras.len();
        self.cameras.push(camera);
        id
    }

    pub fn main_camera(&self) -> &Camera {
        &self.cameras[0]
    }

    pub fn main_camera_mut(&mut self) -> &mut Camera {
        &mut self.cameras[0]
    }

    pub fn get_instance(&self, handle: InstanceHandle) -> Option<&Instance> {
        self.instances.get(handle)
    }

    pub fn get_instance_mut(&mut self, handle: InstanceHandle) -> Option<&mut Instance> {
        self.instances.get_mut(handle)
    }

    pub fn get_meshlet_mesh(&self, id: ResourceHandle<MeshletMesh>) -> Option<Shared<MeshletMesh>> {
        self.asset_server.get(id)
    }

    pub fn get_material(&self, id: ResourceHandle<Material>) -> Shared<Material> {
        self.asset_server
            .get_with_default(id, Material::DEFAULT_MATERIAL_ID)
    }

    pub fn prepare_meshlet_resources(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Option<MeshletBindGroups> {
        if self.instances.scene_instance_count == 0 || self.meshlets.meshlet_count() == 0 {
            return None;
        }

        self.instances.instance_uniforms.write_buffer(device, queue);
        self.instances
            .instance_material_ids
            .write_buffer(device, queue);

        self.meshlets.perform_pending_uploads(device, queue);

        Some(
            self.resources
                .bind_groups(device, &self.meshlets, &self.instances),
        )
    }

    pub fn resources(&self) -> &ResourceStorage {
        &self.resources
    }
}

fn background_load_task(
    path: &str,
    gltf_asset: GltfAsset,
    device: wgpu::Device,
    queue: wgpu::Queue,
    mut asset_server: AssetServer,
) -> anyhow::Result<()> {
    let mut mere_meshes = load_mere_meshes(&path)?.into_iter();

    for model in gltf_asset.models() {
        let default_name = format!("{path}_model_{}", model.index());
        let name = model.name().unwrap_or(&default_name);
        mere_meshes
            .by_ref()
            .zip(model.primitives())
            .for_each(|(mesh, primitive)| {
                match MeshletMesh::load((name, primitive.index() as u32, mesh)) {
                    Ok(mesh) => {
                        asset_server.add(mesh);
                    }
                    Err(err) => {
                        mere_log::error!("Failed to load mesh {name}_{}: {err}", primitive.index())
                    }
                };
            });
    }

    for material in gltf_asset.materials() {
        match Material::load((path, material.clone(), &device, &asset_server)) {
            Ok(material) => {
                asset_server.add(material);
            }
            Err(err) => mere_log::error!("Failed to load material: {:?}: {err}", material.name()),
        }
    }

    const DEFAULT_IMAGE_LOAD_MIP: u32 = 1;
    gltf_asset.images().par_iter().for_each(|image| {
        let uri = match image.source() {
            gltf::image::Source::Uri { uri, .. } => uri,
            gltf::image::Source::View { .. } => {
                mere_log::error!("Unsupported image source");
                return;
            }
        };

        let options = texture_options(uri);
        let image_path = path::PathBuf::from(ASSET_DIR).join(&path).join(uri);

        match Texture::load((
            &image_path,
            &device,
            &queue,
            options,
            DEFAULT_IMAGE_LOAD_MIP,
        )) {
            Ok(tex) => {
                asset_server.clone().add(tex);
            }
            Err(err) => mere_log::error!("Failed to load texture {uri}: {err}"),
        };
    });

    mere_log::success!("Loaded {path}");

    Ok(())
}

fn texture_options(uri: &str) -> TextureOptions {
    const DIFFUSE_ANISOTROPY: u16 = 8;
    const OTHER_ANISOTROPY: u16 = 8;

    let lower = uri.to_lowercase();
    if lower.contains("normal") || (lower.contains("roughness") && lower.contains("metalness")) {
        TextureOptions::texture(wgpu::TextureFormat::Rgba8Unorm)
            .with_mipmap(MipmapOptions::auto(Some(OTHER_ANISOTROPY)))
    } else {
        TextureOptions::texture(wgpu::TextureFormat::Rgba8UnormSrgb)
            .with_mipmap(MipmapOptions::auto(Some(DIFFUSE_ANISOTROPY)))
    }
}
