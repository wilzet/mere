use crate::handle::ResourceHandle;
use image::GenericImageView;

#[derive(Clone, Debug)]
pub struct Texture {
    label: String,
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl Texture {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub fn create_depth_texture(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        label: &str,
    ) -> Self {
        let size = wgpu::Extent3d {
            width: config.width.max(1),
            height: config.height.max(1),
            depth_or_array_layers: 1,
        };

        let desc = wgpu::TextureDescriptor {
            label: Some(label),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };

        let texture = device.create_texture(&desc);

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::LessEqual),
            lod_min_clamp: 0.0,
            lod_max_clamp: 100.0,
            ..Default::default()
        });

        Self {
            label: label.to_string(),
            texture,
            view,
            sampler,
        }
    }

    pub fn from_bytes(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        bytes: &[u8],
        label: &str,
        format: wgpu::TextureFormat,
    ) -> anyhow::Result<Self> {
        Self::from_bytes_with_options(
            device,
            queue,
            width,
            height,
            bytes,
            label,
            format,
            wgpu::AddressMode::Repeat,
            wgpu::FilterMode::Linear,
            wgpu::FilterMode::Nearest,
            wgpu::MipmapFilterMode::Nearest,
        )
    }

    pub fn from_bytes_with_options(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        bytes: &[u8],
        label: &str,
        format: wgpu::TextureFormat,
        address_mode: wgpu::AddressMode,
        mag_filter: wgpu::FilterMode,
        min_filter: wgpu::FilterMode,
        mipmap_filter: wgpu::MipmapFilterMode,
    ) -> anyhow::Result<Self> {
        let image = match image::ImageBuffer::from_raw(width, height, bytes.to_vec()) {
            Some(buffer) => buffer,
            None => anyhow::bail!("invalid dimensions"),
        };

        Self::from_image_with_options(
            device,
            queue,
            image::DynamicImage::ImageRgba8(image),
            label,
            format,
            address_mode,
            mag_filter,
            min_filter,
            mipmap_filter,
        )
    }

    pub fn from_image(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        image: image::DynamicImage,
        label: &str,
        format: wgpu::TextureFormat,
    ) -> anyhow::Result<Self> {
        Self::from_image_with_options(
            device,
            queue,
            image,
            label,
            format,
            wgpu::AddressMode::Repeat,
            wgpu::FilterMode::Linear,
            wgpu::FilterMode::Nearest,
            wgpu::MipmapFilterMode::Nearest,
        )
    }

    pub fn from_image_with_options(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        image: image::DynamicImage,
        label: &str,
        format: wgpu::TextureFormat,
        address_mode: wgpu::AddressMode,
        mag_filter: wgpu::FilterMode,
        min_filter: wgpu::FilterMode,
        mipmap_filter: wgpu::MipmapFilterMode,
    ) -> anyhow::Result<Self> {
        use wgpu::TextureFormat as TF;

        let (data, bytes_per_pixel) = match format {
            TF::R8Unorm | TF::R8Snorm | TF::R8Uint | TF::R8Sint => (image.to_luma8().into_raw(), 1),
            TF::Rg8Unorm | TF::Rg8Snorm | TF::Rg8Uint | TF::Rg8Sint => {
                (image.to_luma_alpha8().into_raw(), 2)
            }
            TF::Rgba8UnormSrgb
            | TF::Rgba8Unorm
            | TF::Rgba8Snorm
            | TF::Rgba8Uint
            | TF::Rgba8Sint => (image.to_rgba8().into_raw(), 4),
            _ => anyhow::bail!("invalid texture format {:?}", format),
        };

        let (width, height) = image.dimensions();

        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                aspect: wgpu::TextureAspect::All,
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            &data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_pixel * width),
                rows_per_image: Some(height),
            },
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: address_mode,
            address_mode_v: address_mode,
            address_mode_w: address_mode,
            mag_filter,
            min_filter,
            mipmap_filter,
            ..Default::default()
        });

        Ok(Self {
            label: label.to_string(),
            texture,
            view,
            sampler,
        })
    }

    pub fn bind_group_entry_view(&self, binding: u32) -> wgpu::BindGroupEntry<'_> {
        wgpu::BindGroupEntry {
            binding,
            resource: wgpu::BindingResource::TextureView(&self.view),
        }
    }

    pub fn bind_group_entry_sampler(&self, binding: u32) -> wgpu::BindGroupEntry<'_> {
        wgpu::BindGroupEntry {
            binding,
            resource: wgpu::BindingResource::Sampler(&self.sampler),
        }
    }

    pub fn label(&self) -> &str {
        &self.label
    }
}

impl Texture {
    pub const DEFAULT_WHITE_TEXTURE_ID: ResourceHandle<Self> = ResourceHandle::new(0);
    pub const DEFAULT_BLACK_TEXTURE_ID: ResourceHandle<Self> = ResourceHandle::new(1);
    pub const DEFAULT_CHEQUERED_TEXTURE_ID: ResourceHandle<Self> = ResourceHandle::new(2);

    pub const DEFAULT_TEXTURES: [(ResourceHandle<Self>, &str, [u8; 16]); 3] = [
        (
            Self::DEFAULT_WHITE_TEXTURE_ID,
            "mere_default_white_texture",
            [0xff; 16],
        ),
        (
            Self::DEFAULT_BLACK_TEXTURE_ID,
            "mere_default_black_texture",
            [0; 16],
        ),
        (
            Self::DEFAULT_CHEQUERED_TEXTURE_ID,
            "mere_default_chequered_texture",
            [
                0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff, 0xff, 0xff,
            ],
        ),
    ];
}
