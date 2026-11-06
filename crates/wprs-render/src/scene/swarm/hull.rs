use image::RgbaImage;

const LEAST: f32 = 0.9;
const PAD: f32 = 0.02;

pub(super) fn hull(image: &RgbaImage, [cols, rows]: [u32; 2]) -> Option<[[f32; 4]; 2]> {
    let (cw, ch) = (image.width() / cols.max(1), image.height() / rows.max(1));
    if cw == 0 || ch == 0 {
        return None;
    }
    let (mut lo, mut hi) = ([f32::MAX; 4], [f32::MIN; 4]);
    for (x, y, _) in image.enumerate_pixels().filter(|(_, _, p)| p.0[3] > 0) {
        let (u0, v0) = ((x % cw) as f32 / cw as f32, (y % ch) as f32 / ch as f32);
        let (u1, v1) = (u0 + 1.0 / cw as f32, v0 + 1.0 / ch as f32);
        let here = ([u0, v0, u0 + v0, u0 - v1], [u1, v1, u1 + v1, u1 - v0]);
        (0..4).for_each(|k| (lo[k], hi[k]) = (lo[k].min(here.0[k]), hi[k].max(here.1[k])));
    }
    if lo[0] > hi[0] {
        return None;
    }
    let pad = [PAD + 1.5 / cw as f32, PAD + 1.5 / ch as f32];
    let side = pad[0] + pad[1];
    let (x0, y0) = ((lo[0] - pad[0]).max(0.0), (lo[1] - pad[1]).max(0.0));
    let (x1, y1) = ((hi[0] + pad[0]).min(1.0), (hi[1] + pad[1]).min(1.0));
    let (s0, s1) = ((lo[2] - side).max(x0 + y0), (hi[2] + side).min(x1 + y1));
    let (d0, d1) = ((lo[3] - side).max(x0 - y1), (hi[3] + side).min(x1 - y0));
    let cuts = [s0 - x0 - y0, x1 + y1 - s1, d0 - x0 + y1, x1 - y0 - d1];
    let area = (x1 - x0) * (y1 - y0) - cuts.iter().map(|c| 0.5 * c * c).sum::<f32>();
    (area < LEAST).then_some([[x0, y0, s0, d0], [x1, y1, s1, d1]])
}

#[cfg(test)]
mod tests {
    use image::{Rgba, RgbaImage};

    #[test]
    fn a_round_sprite_is_drawn_inside_an_octagon_that_keeps_every_visible_texel() {
        let disc = RgbaImage::from_fn(64, 64, |x, y| {
            let d = ((x as f32 - 31.5).powi(2) + (y as f32 - 31.5).powi(2)).sqrt();
            Rgba([255, 255, 255, if d < 24.0 { 200 } else { 0 }])
        });
        let [[x0, y0, s0, d0], [x1, y1, s1, d1]] = super::hull(&disc, [1, 1]).expect("an octagon");
        for (x, y, p) in disc.enumerate_pixels().filter(|(_, _, p)| p.0[3] > 0) {
            for (u, v) in [(x, y), (x + 1, y), (x, y + 1), (x + 1, y + 1)]
                .map(|(a, b)| (a as f32 / 64.0, b as f32 / 64.0))
            {
                assert!(
                    u >= x0 && u <= x1 && v >= y0 && v <= y1 && u + v >= s0 && u + v <= s1,
                    "{x} {y} {p:?}"
                );
                assert!(u - v >= d0 && u - v <= d1, "{x} {y}");
            }
        }
        let full = RgbaImage::from_pixel(8, 8, Rgba([255; 4]));
        assert!(super::hull(&full, [1, 1]).is_none());
    }
}
