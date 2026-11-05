use crate::{Layer, Renderer};
use image::RgbaImage;
use wprs_core::{Fit, UvRect};
use wprs_ktx2::Target;

impl Renderer {
    pub fn look_ktx2(&self, bytes: &[u8], target: Target) -> Option<RgbaImage> {
        if !target.compressed() || !self.ktx2_targets().contains(&target) {
            return None;
        }
        let made = wprs_ktx2::levels(bytes, target, self.max_edge(), false).ok()?;
        let image = self.scoped(|| self.packed_image(&made, false)).ok()?;
        let mut drawn = self.create_target(made.width, made.height).ok()?;
        let uv = UvRect { offset: [0.0; 2], scale: image.crop, repeat: false };
        let layer = Layer {
            image: Some(&image),
            fit: Fit::Stretch,
            background: [0; 3],
            opacity: 1.0,
            uv: Some(uv),
            relief: None,
            flip: [false; 2],
            grade: None,
        };
        let mut bgra = Vec::new();
        self.render_layers(&mut drawn, &[layer], &mut bgra).ok()?;
        let rgba = bgra.as_chunks::<4>().0.iter().flat_map(|p| [p[2], p[1], p[0], 255]).collect();
        RgbaImage::from_raw(made.width, made.height, rgba)
    }
}
