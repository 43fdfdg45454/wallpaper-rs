use crate::readback::{copy_rows, padded_row_bytes};
use crate::texture::GpuImage;
use crate::{RenderError, Renderer};
use image::RgbaImage;

impl Renderer {
    pub fn read_image(&self, image: &GpuImage) -> Result<RgbaImage, RenderError> {
        let (width, height) = image.size();
        self.check_target(width, height)?;
        let stride = padded_row_bytes(width);
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("wprs-render image readback"),
            size: u64::from(stride) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let layout = wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(stride),
            rows_per_image: Some(height),
        };
        let copy = wgpu::TexelCopyBufferInfo { buffer: &buffer, layout };
        encoder.copy_texture_to_buffer(image.texture.as_image_copy(), copy, image.texture.size());
        let done = self.scoped(|| self.queue.submit([encoder.finish()]))?;
        let slice = buffer.slice(..);
        self.map(&slice, &Default::default(), done)?;
        let mut pixels = Vec::new();
        let view = slice.get_mapped_range().map_err(|e| RenderError::Readback(e.to_string()))?;
        copy_rows(&view, &mut pixels, width, height, stride);
        RgbaImage::from_raw(width, height, pixels).ok_or_else(|| RenderError::Readback("short image".into()))
    }
}
