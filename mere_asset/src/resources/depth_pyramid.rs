use crate::{
    resources::render_resources::{Layout, PerFrameResources, entry, storage_texture},
    texture::{Texture, TextureOptions},
};
use mere_log::Profiler;

const DEPTH_PYRAMID_COUNT: usize = 12;

#[derive(Clone, Debug)]
pub struct DepthPyramid {
    pub depth_pyramid: Texture,
    pub depth_pyramid_mips: [wgpu::TextureView; DEPTH_PYRAMID_COUNT],
    pub downsample_depth_first_pipeline: wgpu::ComputePipeline,
    pub downsample_depth_second_pipeline: wgpu::ComputePipeline,
    pub downsample_depth_bind_group_layout: wgpu::BindGroupLayout,
    pub mip_count: u32,
}

impl DepthPyramid {
    pub fn new(device: &wgpu::Device, label: &str, width: u32, height: u32) -> Self {
        let size = wgpu::Extent3d {
            width: (width + 1).next_power_of_two() / 2,
            height: (height + 1).next_power_of_two() / 2,
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

        let dummy_texture = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("depth_mips_dummy_texture"),
                size: wgpu::Extent3d::default(),
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R32Float,
                usage: wgpu::TextureUsages::STORAGE_BINDING,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor {
                label: Some("depth_mips_dummy_texture_view"),
                format: Some(wgpu::TextureFormat::R32Float),
                dimension: Some(wgpu::TextureViewDimension::D2),
                usage: None,
                aspect: wgpu::TextureAspect::All,
                base_mip_level: 0,
                mip_level_count: Some(1),
                base_array_layer: 0,
                array_layer_count: Some(1),
            });

        let depth_pyramid_mips = std::array::from_fn(|i| {
            if (i as u32) < mip_count {
                depth_pyramid
                    .texture()
                    .create_view(&wgpu::TextureViewDescriptor {
                        label: Some(&format!("{label}_mip_{i}")),
                        format: Some(wgpu::TextureFormat::R32Float),
                        dimension: Some(wgpu::TextureViewDimension::D2),
                        usage: None,
                        aspect: wgpu::TextureAspect::All,
                        base_mip_level: i as u32,
                        mip_level_count: Some(1),
                        base_array_layer: 0,
                        array_layer_count: Some(1),
                    })
            } else {
                dummy_texture.clone()
            }
        });

        let downsample_depth_bind_group_layout = device.create_bind_group_layout(
            &Layout::sequential(
                Some("downsample_depth_bind_group_layout"),
                wgpu::ShaderStages::COMPUTE,
                &mut [
                    storage_texture(
                        wgpu::TextureFormat::R64Uint,
                        wgpu::StorageTextureAccess::ReadOnly,
                    ),
                    storage_texture(
                        wgpu::TextureFormat::R32Float,
                        wgpu::StorageTextureAccess::WriteOnly,
                    ),
                    storage_texture(
                        wgpu::TextureFormat::R32Float,
                        wgpu::StorageTextureAccess::WriteOnly,
                    ),
                    storage_texture(
                        wgpu::TextureFormat::R32Float,
                        wgpu::StorageTextureAccess::WriteOnly,
                    ),
                    storage_texture(
                        wgpu::TextureFormat::R32Float,
                        wgpu::StorageTextureAccess::WriteOnly,
                    ),
                    storage_texture(
                        wgpu::TextureFormat::R32Float,
                        wgpu::StorageTextureAccess::WriteOnly,
                    ),
                    storage_texture(
                        wgpu::TextureFormat::R32Float,
                        wgpu::StorageTextureAccess::ReadWrite,
                    ),
                    storage_texture(
                        wgpu::TextureFormat::R32Float,
                        wgpu::StorageTextureAccess::WriteOnly,
                    ),
                    storage_texture(
                        wgpu::TextureFormat::R32Float,
                        wgpu::StorageTextureAccess::WriteOnly,
                    ),
                    storage_texture(
                        wgpu::TextureFormat::R32Float,
                        wgpu::StorageTextureAccess::WriteOnly,
                    ),
                    storage_texture(
                        wgpu::TextureFormat::R32Float,
                        wgpu::StorageTextureAccess::WriteOnly,
                    ),
                    storage_texture(
                        wgpu::TextureFormat::R32Float,
                        wgpu::StorageTextureAccess::WriteOnly,
                    ),
                    storage_texture(
                        wgpu::TextureFormat::R32Float,
                        wgpu::StorageTextureAccess::WriteOnly,
                    ),
                    entry(wgpu::BindingType::Sampler(
                        wgpu::SamplerBindingType::NonFiltering,
                    )),
                ],
            )
            .get(),
        );

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("downsample_depth_pipeline_layout"),
            bind_group_layouts: &[Some(&downsample_depth_bind_group_layout)],
            immediate_size: 4,
        });

        let downsample_depth_first_pipeline = create_compute_pipeline(
            Some("downsample_depth_pipeline"),
            device,
            Some(&layout),
            wgpu::include_wgsl!("downsample_depth.wgsl"),
            "downsample_depth_first",
        );

        let downsample_depth_second_pipeline = create_compute_pipeline(
            Some("downsample_depth_pipeline"),
            device,
            Some(&layout),
            wgpu::include_wgsl!("downsample_depth.wgsl"),
            "downsample_depth_second",
        );

        Self {
            depth_pyramid,
            depth_pyramid_mips,
            downsample_depth_first_pipeline,
            downsample_depth_second_pipeline,
            downsample_depth_bind_group_layout,
            mip_count,
        }
    }

    pub fn virtual_size(&self) -> (u32, u32) {
        let size = self.depth_pyramid.texture().size();
        let virtual_view_size_x = (size.width + 1).next_power_of_two();
        let virtual_view_size_y = (size.height + 1).next_power_of_two();

        (virtual_view_size_x, virtual_view_size_y)
    }

    pub fn downsample(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        resources: &PerFrameResources,
        profiler: &mut Profiler,
    ) {
        let timestamp_writes = profiler.begin("depth_pyramid_downsample");

        let mut downsample_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("depth_pyramid_downsample"),
            timestamp_writes,
        });
        downsample_pass.set_pipeline(&self.downsample_depth_first_pipeline);
        downsample_pass.set_bind_group(0, &resources.bind_groups.downsample_depth_bind_group, &[]);
        downsample_pass.set_immediates(0, &self.mip_count.to_le_bytes());

        let (width, height) = self.virtual_size();
        downsample_pass.dispatch_workgroups(width.div_ceil(64), height.div_ceil(64), 1);

        if self.mip_count >= 7 {
            downsample_pass.set_pipeline(&self.downsample_depth_second_pipeline);
            downsample_pass.dispatch_workgroups(1, 1, 1);
        }

        profiler.end();
    }
}

fn create_compute_pipeline(
    label: Option<&str>,
    device: &wgpu::Device,
    layout: Option<&wgpu::PipelineLayout>,
    shader: wgpu::ShaderModuleDescriptor,
    entry_point: &str,
) -> wgpu::ComputePipeline {
    let shader = device.create_shader_module(shader);

    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label,
        layout,
        module: &shader,
        entry_point: Some(entry_point),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    })
}
