use crate::SceneView;
use crate::{RenderError, Renderer, Surface};
use wgpu::{
    SurfaceCapabilities, SurfaceColorSpace as Space, SurfaceColorSpaces as Spaces, TextureFormat as Format,
};

pub const MAX_HEADROOM: f32 = 64.0;

pub fn lit(caps: &SurfaceCapabilities, wanted: bool) -> (bool, Option<(Format, Space)>) {
    let linear = |f: &&wgpu::SurfaceFormatCapabilities| {
        f.format == Format::Rgba16Float && f.color_spaces.contains(Spaces::EXTENDED_SRGB_LINEAR)
    };
    let offered = caps.format_capabilities.iter().any(|f| linear(&f));
    (offered, (offered && wanted).then_some((Format::Rgba16Float, Space::ExtendedSrgbLinear)))
}

impl crate::GpuImage {
    pub fn format(&self) -> Format {
        self.texture.format()
    }
}

impl Surface {
    pub fn hdr(&self) -> bool {
        self.config.color_space == Space::ExtendedSrgbLinear
    }

    pub fn hdr_offered(&self) -> bool {
        self.offered
    }
}

impl Renderer {
    pub fn light(&self, target: &mut Surface, on: bool) -> Result<bool, RenderError> {
        if target.presentation.hdr == on {
            return Ok(false);
        }
        let before = target.hdr();
        target.presentation.hdr = on;
        self.renew(target)?;
        Ok(target.hdr() != before)
    }
}

impl Renderer {
    pub fn read_light(&self, image: &crate::GpuImage) -> Result<Vec<[f32; 4]>, RenderError> {
        let (width, height) = image.size();
        let wide = image.texture.format() == Format::Rgba16Float;
        if !wide || width == 0 || height == 0 || width > 4096 || height > 4096 {
            return Err(RenderError::Readback("only half float pictures up to 4096 px are read".into()));
        }
        let stride = (width * 8).div_ceil(256) * 256;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("wprs-render light readback"),
            size: u64::from(stride) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let layout =
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(stride), rows_per_image: None };
        let copy = wgpu::TexelCopyBufferInfo { buffer: &buffer, layout };
        encoder.copy_texture_to_buffer(image.texture.as_image_copy(), copy, image.texture.size());
        let done = self.scoped(|| self.queue.submit([encoder.finish()]))?;
        let slice = buffer.slice(..);
        self.map(&slice, &Default::default(), done)?;
        let mapped = slice.get_mapped_range().map_err(|e| RenderError::Readback(e.to_string()))?;
        let half = |b: &[u8]| half::f16::from_le_bytes([b[0], b[1]]).to_f32();
        let row = |y: u32| mapped[(y * stride) as usize..][..(width * 8) as usize].as_chunks::<8>().0;
        let texel = |t: &[u8; 8]| [half(&t[0..2]), half(&t[2..4]), half(&t[4..6]), half(&t[6..8])];
        Ok((0..height).flat_map(|y| row(y).iter().map(texel).collect::<Vec<_>>()).collect())
    }
}

impl SceneView {
    pub fn light(&mut self, headroom: f32) {
        let headroom = if headroom.is_finite() { headroom.clamp(1.0, MAX_HEADROOM) } else { 1.0 };
        if (headroom - self.headroom).abs() > 1e-3 {
            self.headroom = headroom;
            self.marks.clear();
        }
    }

    pub fn headroom(&self) -> f32 {
        self.headroom
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hdr_needs_a_linear_half_float_surface() {
        let with = |format, color_spaces| SurfaceCapabilities {
            format_capabilities: vec![wgpu::SurfaceFormatCapabilities { format, color_spaces }],
            ..Default::default()
        };
        let scrgb = with(Format::Rgba16Float, Spaces::SRGB | Spaces::EXTENDED_SRGB_LINEAR);
        assert_eq!(lit(&scrgb, true), (true, Some((Format::Rgba16Float, Space::ExtendedSrgbLinear))));
        assert_eq!(lit(&scrgb, false), (true, None));
        assert_eq!(lit(&with(Format::Rgba16Float, Spaces::SRGB), true), (false, None));
        assert_eq!(lit(&with(Format::Rgb10a2Unorm, Spaces::BT2100_PQ), true), (false, None));
        assert_eq!(lit(&SurfaceCapabilities::default(), true), (false, None));
    }
}
