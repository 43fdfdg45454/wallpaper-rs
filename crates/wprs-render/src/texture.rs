use crate::device::create_texture;
use crate::{RenderError, Renderer};
use image::RgbaImage;
use std::borrow::Cow;
use wprs_core::{Fit, UvRect};

pub struct GpuImage {
    pub(crate) texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
}

impl GpuImage {
    pub fn size(&self) -> (u32, u32) {
        (self.texture.width(), self.texture.height())
    }
}

impl Renderer {
    pub fn upload(&self, image: &RgbaImage, fit: Fit, target: (u32, u32)) -> GpuImage {
        let prepared = prepare_image(image, fit, target, self.max_edge());
        let (w, h) = prepared.dimensions();
        create_image(&self.device, &self.queue, prepared.as_raw(), w, h, w * 4)
    }

    pub fn upload_rgba(
        &self,
        rgba: &[u8],
        width: u32,
        height: u32,
        stride: u32,
    ) -> Result<GpuImage, RenderError> {
        self.check_pixels(rgba, width, height, stride)?;
        Ok(create_image(&self.device, &self.queue, rgba, width, height, stride))
    }

    pub fn update_rgba(
        &self,
        image: &mut GpuImage,
        rgba: &[u8],
        width: u32,
        height: u32,
        stride: u32,
    ) -> Result<(), RenderError> {
        self.check_pixels(rgba, width, height, stride)?;
        if image.size() == (width, height) {
            write_image(&self.queue, &image.texture, rgba, stride);
        } else {
            *image = create_image(&self.device, &self.queue, rgba, width, height, stride);
        }
        Ok(())
    }

    fn check_pixels(&self, rgba: &[u8], width: u32, height: u32, stride: u32) -> Result<(), RenderError> {
        let needed = u64::from(stride) * u64::from(height.saturating_sub(1)) + u64::from(width) * 4;
        self.check_size(width, height)?;
        let valid = stride >= width * 4 && rgba.len() as u64 >= needed;
        valid.then_some(()).ok_or(RenderError::InvalidSize(width, height))
    }
}

pub(crate) fn create_image(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    rgba: &[u8],
    width: u32,
    height: u32,
    stride: u32,
) -> GpuImage {
    let usage = wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST;
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let texture = create_texture(device, "wprs-render image", (width, height), format, usage);
    write_image(queue, &texture, rgba, stride);
    GpuImage { view: texture.create_view(&Default::default()), texture }
}

fn write_image(queue: &wgpu::Queue, texture: &wgpu::Texture, rgba: &[u8], stride: u32) {
    let layout = wgpu::TexelCopyBufferLayout {
        offset: 0,
        bytes_per_row: Some(stride),
        rows_per_image: Some(texture.height()),
    };
    queue.write_texture(texture.as_image_copy(), rgba, layout, texture.size());
}

fn prepare_image(img: &RgbaImage, fit: Fit, target: (u32, u32), max_edge: u32) -> Cow<'_, RgbaImage> {
    let (iw, ih) = img.dimensions();
    let uv = UvRect::compute(fit, (iw, ih), target);
    let shown = [target.0 as f32 / uv.scale[0], target.1 as f32 / uv.scale[1]].map(|v| v.ceil().max(1.0));
    let edge = iw.max(ih) as f32;
    let factor = (shown[0] / iw as f32).max(shown[1] / ih as f32).min(1.0);
    let factor = if edge * factor > max_edge as f32 { max_edge as f32 / edge } else { factor };
    if factor >= 0.5 && iw <= max_edge && ih <= max_edge {
        return Cow::Borrowed(img);
    }
    let size = |v: u32| ((v as f32 * factor).round() as u32).max(1);
    Cow::Owned(image::imageops::resize(img, size(iw), size(ih), image::imageops::FilterType::Triangle))
}
