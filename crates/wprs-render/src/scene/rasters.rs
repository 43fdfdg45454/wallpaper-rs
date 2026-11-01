use super::SceneView;
use super::item::scale_of;
use super::text::raster;
use crate::Renderer;
use crate::texture::{GpuImage, create_image};
use std::hash::{DefaultHasher, Hash, Hasher};
use wprs_scene::{Affine, Content, Frame};

pub(crate) struct Raster {
    pub(crate) image: GpuImage,
    pub(crate) size: [f32; 2],
    pub(crate) used: u64,
    pub(crate) field: f32,
}

fn reference(size: f32) -> f32 {
    ((size * 2.0).clamp(16.0, 256.0) * 2.0).round() / 2.0
}

impl Renderer {
    pub(crate) fn rasters(
        &self,
        view: &mut SceneView,
        frame: &Frame<'_>,
        screen: Affine,
        found: &mut Vec<(usize, u64)>,
    ) {
        for (i, d) in frame.draws.iter().enumerate() {
            let Content::Text { text, font, size, align, fallback, layout, ink, .. } = &d.content else {
                continue;
            };
            let k = scale_of(screen.then(d.world)).max(1e-3);
            let fielded = ink.filter(|_| view.state_format == wgpu::TextureFormat::Rgba16Float);
            let px = match fielded {
                Some(ink) if ink.field => reference(*size),
                _ => (size * k * 2.0).round() / 2.0,
            };
            let key = {
                let mut h = DefaultHasher::new();
                let wrap = layout.wrap.map(|w| (w.width.to_bits(), w.lines, w.ellipsis));
                let backing = layout.backing.map(|b| (b.1.map(f32::to_bits), b.2.to_bits()));
                let spacing = (layout.line_height.map(f32::to_bits), layout.padding.to_bits(), backing);
                let styled = (layout.spacing.to_bits(), layout.reach.to_bits(), fielded.is_some());
                (text, font, fallback, px.to_bits(), *align as u8, wrap, spacing, styled).hash(&mut h);
                h.finish()
            };
            let frame_no = view.pool.frame;
            if let Some(entry) = view.texts.get_mut(&key) {
                entry.used = frame_no;
            } else {
                let fonts = view.fonts.chain(*font, fallback);
                if let Some(image) = raster(&fonts, text, px, *align, px / size, layout) {
                    let (w, h) = image.dimensions();
                    let mut gpu = create_image(&self.device, &self.queue, image.as_raw(), w, h, w * 4);
                    if fielded.is_some() {
                        gpu = self.field(&gpu, (layout.reach * px / size).ceil() as u32);
                    }
                    view.pool.created += 1 + 3 * u32::from(fielded.is_some());
                    let field = if fielded.is_some() { px / size } else { 0.0 };
                    let size = [w as f32 * size / px, h as f32 * size / px];
                    view.texts.insert(key, Raster { image: gpu, size, used: frame_no, field });
                }
            }
            if view.texts.contains_key(&key) {
                found.push((i, key));
            }
        }
    }
}
