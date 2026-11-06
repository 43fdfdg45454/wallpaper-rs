use crate::{GpuImage, Layer, Renderer};
use image::RgbaImage;
use wprs_core::{Fit, UvRect};
use wprs_ktx2::Target;
use wprs_ktx2::light::{decoded, encoded};

fn unblended(black: &[u8; 4], white: &[u8; 4]) -> [u8; 4] {
    let (b, w) = ([2, 1, 0].map(|i| decoded(black[i])), [2, 1, 0].map(|i| decoded(white[i])));
    let alpha = (1.0 - (0..3).map(|i| w[i] - b[i]).sum::<f32>() / 3.0).clamp(0.0, 1.0);
    let color = |i: usize| if alpha > 0.0 { encoded(b[i] / alpha) } else { 0 };
    [color(0), color(1), color(2), (alpha * 255.0).round() as u8]
}

impl Renderer {
    fn drawn_over(
        &self,
        image: &GpuImage,
        (width, height): (u32, u32),
        background: [u8; 3],
    ) -> Option<Vec<u8>> {
        let mut drawn = self.create_target(width, height).ok()?;
        let uv = UvRect { offset: [0.0; 2], scale: image.crop, repeat: false };
        let layer = Layer {
            image: Some(image),
            fit: Fit::Stretch,
            background,
            opacity: 1.0,
            uv: Some(uv),
            relief: None,
            flip: [false; 2],
            grade: None,
        };
        let mut bgra = Vec::new();
        self.render_layers(&mut drawn, &[layer], &mut bgra).ok()?;
        Some(bgra)
    }

    pub fn look_ktx2(&self, bytes: &[u8], target: Target) -> Option<RgbaImage> {
        if !target.compressed() || !self.ktx2_targets().contains(&target) {
            return None;
        }
        let made = wprs_ktx2::levels(bytes, target, self.max_edge(), false).ok()?;
        let image = self.scoped(|| self.packed_image(&made, false)).ok()?;
        let size = (made.width, made.height);
        let black = self.drawn_over(&image, size, [0; 3])?;
        let white = self.drawn_over(&image, size, [255; 3])?;
        let pairs = black.as_chunks::<4>().0.iter().zip(white.as_chunks::<4>().0);
        let rgba = pairs.flat_map(|(b, w)| unblended(b, w)).collect();
        RgbaImage::from_raw(made.width, made.height, rgba)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_color_drawn_over_black_and_white_comes_back_with_its_alpha() {
        assert_eq!(super::unblended(&[0, 0, 0, 255], &[255, 255, 255, 255]), [0, 0, 0, 0]);
        assert_eq!(super::unblended(&[10, 20, 200, 255], &[10, 20, 200, 255]), [200, 20, 10, 255]);
        let half = super::unblended(&[0, 0, 188, 255], &[188, 188, 255, 255]);
        assert!((126..=130).contains(&half[3]) && half[0] >= 250 && half[2] == 0, "{half:?}");
    }
}
