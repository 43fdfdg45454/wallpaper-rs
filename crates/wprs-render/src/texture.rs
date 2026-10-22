use crate::decode::prepare_image;
use crate::{RenderError, Renderer};
use image::RgbaImage;
use std::sync::atomic::{AtomicU64, Ordering};
use wprs_core::Fit;

pub struct GpuImage {
    pub(crate) texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
    pub(crate) id: u64,
}

impl GpuImage {
    pub fn size(&self) -> (u32, u32) {
        (self.texture.width(), self.texture.height())
    }
}

impl Renderer {
    pub fn upload(&self, image: &RgbaImage, fit: Fit, target: (u32, u32)) -> Result<GpuImage, RenderError> {
        let prepared = prepare_image(image, fit, target, self.max_edge());
        let (w, h) = prepared.dimensions();
        self.scoped(|| create_image(&self.device, &self.queue, prepared.as_raw(), w, h, w * 4))
    }

    pub fn upload_rgba(
        &self,
        rgba: &[u8],
        width: u32,
        height: u32,
        stride: u32,
    ) -> Result<GpuImage, RenderError> {
        self.check_pixels(rgba, width, height, stride)?;
        self.scoped(|| create_image(&self.device, &self.queue, rgba, width, height, stride))
    }

    pub fn update_rgba(
        &self,
        image: &mut GpuImage,
        rgba: &[u8],
        width: u32,
        height: u32,
        stride: u32,
    ) -> Result<(), RenderError> {
        if image.size() != (width, height) {
            *image = self.upload_rgba(rgba, width, height, stride)?;
            return Ok(());
        }
        self.check_pixels(rgba, width, height, stride)?;
        write_image(&self.queue, &image.texture, rgba, stride);
        Ok(())
    }

    fn check_pixels(&self, rgba: &[u8], width: u32, height: u32, stride: u32) -> Result<(), RenderError> {
        let needed = u64::from(stride) * u64::from(height.saturating_sub(1)) + u64::from(width) * 4;
        let valid = self.limits.fits(width, height) && stride >= width * 4 && rgba.len() as u64 >= needed;
        valid.then_some(()).ok_or(RenderError::InvalidSize(width, height))
    }
}

pub(crate) fn create_texture(
    device: &wgpu::Device,
    label: &str,
    (width, height): (u32, u32),
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    })
}

pub(crate) fn create_image(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    rgba: &[u8],
    width: u32,
    height: u32,
    stride: u32,
) -> GpuImage {
    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    let usage = wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST;
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let texture = create_texture(device, "wprs-render image", (width, height), format, usage);
    write_image(queue, &texture, rgba, stride);
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    GpuImage { view: texture.create_view(&Default::default()), texture, id }
}

fn write_image(queue: &wgpu::Queue, texture: &wgpu::Texture, rgba: &[u8], stride: u32) {
    let layout = wgpu::TexelCopyBufferLayout {
        offset: 0,
        bytes_per_row: Some(stride),
        rows_per_image: Some(texture.height()),
    };
    queue.write_texture(texture.as_image_copy(), rgba, layout, texture.size());
}
