use crate::shader::{Pairs, layout_of, module};
use crate::texture::{GpuImage, next_id};
use crate::{RenderError, Renderer};
use image::RgbaImage;
use std::sync::Arc;
use wgpu::{TextureDimension, TextureFormat, TextureUsages, TextureViewDimension as Dim};

pub const MOST_LAYERS: usize = 8;

impl Renderer {
    pub(crate) fn decks(&self) -> &Pairs {
        self.decks.get_or_init(|| {
            let shader =
                module(&self.device, &[include_str!("grade.wgsl"), include_str!("deck.wgsl")].concat());
            Pairs::new(layout_of(&self.device, &[Dim::D2, Dim::D2Array]), shader, "fs_deck")
        })
    }

    pub fn upload_layers(&self, layers: &[RgbaImage], levels: &[f32]) -> Result<GpuImage, RenderError> {
        let first = layers.first().map(RgbaImage::dimensions).filter(|_| layers.len() == levels.len());
        let (width, height) = first.ok_or(RenderError::InvalidSize(0, 0))?;
        self.check_target(width, height)?;
        if layers.len() > MOST_LAYERS || layers.iter().any(|l| l.dimensions() != (width, height)) {
            return Err(RenderError::InvalidSize(width, height));
        }
        self.scoped(|| {
            let size = wgpu::Extent3d { width, height, depth_or_array_layers: layers.len() as u32 };
            let format = TextureFormat::Rgba8UnormSrgb;
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("wprs-render layers"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format,
                usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                view_formats: &[],
            });
            for (z, layer) in layers.iter().enumerate() {
                let mut at = texture.as_image_copy();
                at.origin.z = z as u32;
                let rows = wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(width * 4),
                    rows_per_image: Some(height),
                };
                let one = wgpu::Extent3d { depth_or_array_layers: 1, ..size };
                self.queue.write_texture(at, layer.as_raw(), rows, one);
            }
            let view = texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(Dim::D2Array),
                ..Default::default()
            });
            let levels = Some(Arc::from(levels));
            GpuImage { texture, view, id: next_id(), format, crop: [1.0; 2], levels }
        })
    }
}
