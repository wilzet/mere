use crate::handle::ResourceHandle;
use image::GenericImageView;
use std::num::NonZero;

#[derive(Clone, Copy, Default, Debug)]
pub enum MipmapLevels {
    /// No mipmaps
    #[default]
    None,
    /// Derive mip levels from texture
    Auto,
    /// Specify mip levels
    Levels(NonZero<u32>),
}

#[derive(Clone, Copy, Debug)]
pub struct MipmapOptions {
    levels: MipmapLevels,
    filter: wgpu::MipmapFilterMode,
    anisotropy: NonZero<u16>,
}

impl MipmapOptions {
    /// Upper bound for automatic mip level generation.
    const MIPMAP_LEVEL_BOUND: u32 = 32;

    pub fn new(
        levels: MipmapLevels,
        filter: wgpu::MipmapFilterMode,
        anisotropy: Option<u16>,
    ) -> Self {
        Self {
            levels,
            filter,
            anisotropy: unsafe {
                // SAFETY: Either non-zero or 1
                NonZero::new_unchecked(anisotropy.map_or(1, |a| a.max(1)))
            },
        }
    }

    pub fn auto(anisotropy: Option<u16>) -> Self {
        let filter = match anisotropy {
            Some(_) => wgpu::MipmapFilterMode::Linear,
            None => wgpu::MipmapFilterMode::Nearest,
        };
        Self::new(MipmapLevels::Auto, filter, anisotropy)
    }

    fn levels(&self) -> u32 {
        match self.levels {
            MipmapLevels::None => 1,
            MipmapLevels::Auto => Self::MIPMAP_LEVEL_BOUND,
            MipmapLevels::Levels(levels) => levels.get(),
        }
    }

    fn filter(&self) -> wgpu::MipmapFilterMode {
        self.filter
    }

    fn anisotropy(&self) -> u16 {
        self.anisotropy.get()
    }

    fn try_auto_generate(&mut self, width: u32, height: u32) {
        let extent = width.max(height);
        let mip_level_count = (extent as f32).log2().floor() as u32 + 1;
        let target_levels = self.levels().min(mip_level_count);

        // SAFETY: Even in worst case (width & height equals 0), +1 ensures non-zero. Also Self::levels returns non zero
        self.levels = MipmapLevels::Levels(unsafe { NonZero::new_unchecked(target_levels) });
    }
}

impl Default for MipmapOptions {
    fn default() -> Self {
        Self {
            levels: MipmapLevels::None,
            filter: wgpu::MipmapFilterMode::Linear,
            anisotropy: unsafe {
                // SAFETY: 1 is not 0 (trust me)
                NonZero::new_unchecked(1)
            },
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TextureOptions {
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
    address_mode: wgpu::AddressMode,
    mag_filter: wgpu::FilterMode,
    min_filter: wgpu::FilterMode,
    mipmap: MipmapOptions,
}

impl TextureOptions {
    fn depth() -> Self {
        Self {
            format: Texture::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        }
    }

    pub fn texture(format: wgpu::TextureFormat) -> Self {
        Self {
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            address_mode: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        }
    }

    pub fn with_format(self, format: wgpu::TextureFormat) -> Self {
        Self { format, ..self }
    }

    pub fn with_usage(self, usage: wgpu::TextureUsages) -> Self {
        Self { usage, ..self }
    }

    pub fn with_address_mode(self, address_mode: wgpu::AddressMode) -> Self {
        Self {
            address_mode,
            ..self
        }
    }

    pub fn with_mipmap(self, mipmap: MipmapOptions) -> Self {
        Self { mipmap, ..self }
    }

    pub fn with_mag_min_filter(self, mag: wgpu::FilterMode, min: wgpu::FilterMode) -> Self {
        Self {
            mag_filter: mag,
            min_filter: min,
            ..self
        }
    }
}

impl Default for TextureOptions {
    fn default() -> Self {
        Self {
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::empty(),
            address_mode: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap: MipmapOptions::default(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Texture {
    pub label: String,
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
        let options = TextureOptions::depth();

        let size = wgpu::Extent3d {
            width: config.width.max(1),
            height: config.height.max(1),
            depth_or_array_layers: 1,
        };

        let desc = wgpu::TextureDescriptor {
            label: Some(label),
            size,
            mip_level_count: options.mipmap.levels(),
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: options.format,
            usage: options.usage,
            view_formats: &[],
        };

        let texture = device.create_texture(&desc);

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: options.address_mode,
            address_mode_v: options.address_mode,
            address_mode_w: options.address_mode,
            mag_filter: options.mag_filter,
            min_filter: options.min_filter,
            mipmap_filter: options.mipmap.filter(),
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        Self {
            label: label.to_string(),
            view,
            sampler,
        }
    }

    fn from_bytes_to_raw_image(
        width: u32,
        height: u32,
        bytes: Vec<u8>,
    ) -> anyhow::Result<image::ImageBuffer<image::Rgba<u8>, Vec<u8>>> {
        match image::ImageBuffer::from_raw(width, height, bytes) {
            Some(buffer) => Ok(buffer),
            None => anyhow::bail!("invalid dimensions"),
        }
    }

    fn from_image_to_bytes(
        image: image::DynamicImage,
        format: wgpu::TextureFormat,
    ) -> anyhow::Result<(Vec<u8>, u32)> {
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

        Ok((data, bytes_per_pixel))
    }

    pub fn from_bytes(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        bytes: Vec<u8>,
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
            TextureOptions::texture(format),
        )
    }

    pub fn from_bytes_with_options(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        bytes: Vec<u8>,
        label: &str,
        options: TextureOptions,
    ) -> anyhow::Result<Self> {
        let image = Self::from_bytes_to_raw_image(width, height, bytes)?;

        Self::from_image(
            device,
            queue,
            image::DynamicImage::ImageRgba8(image),
            label,
            options,
        )
    }

    pub fn from_image(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        image: image::DynamicImage,
        label: &str,
        mut options: TextureOptions,
    ) -> anyhow::Result<Self> {
        let (width, height) = image.dimensions();
        options.mipmap.try_auto_generate(width, height);

        let (mut data, bytes_per_pixel) = Self::from_image_to_bytes(image, options.format)?;

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: options.mipmap.levels(),
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: options.format,
            usage: options.usage,
            view_formats: &[],
        });

        let (mut w, mut h) = (width, height);
        for mip_level in 0..options.mipmap.levels() {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    aspect: wgpu::TextureAspect::All,
                    texture: &texture,
                    mip_level,
                    origin: wgpu::Origin3d::ZERO,
                },
                &data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_pixel * w),
                    rows_per_image: Some(h),
                },
                wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
            );

            // prepare next mip level
            if w > 1 || h > 1 {
                let image_buffer = Self::from_bytes_to_raw_image(w, h, data)?;

                (w, h) = ((w / 2).max(1), (h / 2).max(1));
                data = image::imageops::resize(
                    &image_buffer,
                    w,
                    h,
                    image::imageops::FilterType::Triangle,
                )
                .into_raw();
            }
        }

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: options.address_mode,
            address_mode_v: options.address_mode,
            address_mode_w: options.address_mode,
            mag_filter: options.mag_filter,
            min_filter: options.min_filter,
            mipmap_filter: options.mipmap.filter(),
            anisotropy_clamp: options.mipmap.anisotropy(),
            ..Default::default()
        });

        Ok(Self {
            label: label.to_string(),
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
}

impl Texture {
    pub const DEFAULT_WHITE_TEXTURE_ID: ResourceHandle<Self> = ResourceHandle::new(0);
    pub const DEFAULT_CHEQUERED_TEXTURE_ID: ResourceHandle<Self> = ResourceHandle::new(1);

    pub const DEFAULT_TEXTURES: [(ResourceHandle<Self>, &str, [u8; 16]); 2] = [
        (
            Self::DEFAULT_WHITE_TEXTURE_ID,
            "mere_default_white_texture",
            [0xff; 16],
        ),
        (
            Self::DEFAULT_CHEQUERED_TEXTURE_ID,
            "mere_default_chequered_texture",
            [
                0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0xff, 0, 0, 0, 0xff, 0xff, 0xff, 0xff, 0xff,
            ],
        ),
    ];
}
