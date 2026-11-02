use image::{Rgba, RgbaImage, imageops};

pub const MAX_STRENGTH: f32 = 16.0;
pub const MAX_BLUR: f32 = 32.0;

fn heights(picture: &RgbaImage, blur: f32) -> image::ImageBuffer<image::Luma<f32>, Vec<f32>> {
    let lum = image::ImageBuffer::from_fn(picture.width(), picture.height(), |x, y| {
        let [r, g, b, a] = picture.get_pixel(x, y).0.map(f32::from);
        image::Luma([(0.2126 * r + 0.7152 * g + 0.0722 * b) * a / 65_025.0])
    });
    if blur > 0.0 { imageops::blur(&lum, blur.min(MAX_BLUR)) } else { lum }
}

pub fn normals(picture: &RgbaImage, strength: f32, blur: f32) -> RgbaImage {
    let h = heights(picture, blur);
    let (w, ht) = h.dimensions();
    let s = strength.clamp(0.0, MAX_STRENGTH);
    let at =
        |x: i64, y: i64| h.get_pixel(x.clamp(0, w as i64 - 1) as u32, y.clamp(0, ht as i64 - 1) as u32).0[0];
    RgbaImage::from_fn(w, ht, |x, y| {
        let (x, y) = (i64::from(x), i64::from(y));
        let column = |dx: i64| at(x + dx, y - 1) + 2.0 * at(x + dx, y) + at(x + dx, y + 1);
        let row = |dy: i64| at(x - 1, y + dy) + 2.0 * at(x, y + dy) + at(x + 1, y + dy);
        let (gx, gy) = (column(1) - column(-1), row(1) - row(-1));
        let n = [-gx * s, gy * s, 1.0];
        let long = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        let byte = |v: f32| ((v / long * 0.5 + 0.5) * 255.0).round().clamp(0.0, 255.0) as u8;
        Rgba([byte(n[0]), byte(n[1]), byte(n[2]), 255])
    })
}

pub fn relief(normals: &RgbaImage, light: [f32; 3]) -> RgbaImage {
    let long = (light[0] * light[0] + light[1] * light[1] + light[2] * light[2]).sqrt().max(1e-6);
    let l = light.map(|v| v / long);
    RgbaImage::from_fn(normals.width(), normals.height(), |x, y| {
        let [r, g, b, _] = normals.get_pixel(x, y).0.map(|v| f32::from(v) / 127.5 - 1.0);
        let k = (r * l[0] + g * l[1] + b * l[2]).max(0.0);
        let v = (40.0 + 215.0 * k).round() as u8;
        Rgba([v, v, v, 255])
    })
}

pub fn sphere(side: u32, light: [f32; 3]) -> RgbaImage {
    let r = side as f32 / 2.0;
    let flat = RgbaImage::from_fn(side, side, |x, y| {
        let (u, v) = ((x as f32 + 0.5 - r) / r, (r - y as f32 - 0.5) / r);
        let z2 = 1.0 - u * u - v * v;
        let byte = |c: f32| ((c * 0.5 + 0.5) * 255.0).round() as u8;
        if z2 < 0.0 { Rgba([128, 128, 255, 0]) } else { Rgba([byte(u), byte(v), byte(z2.sqrt()), 255]) }
    });
    let mut shaded = relief(&flat, light);
    shaded.pixels_mut().zip(flat.pixels()).for_each(|(p, f)| p.0[3] = f.0[3]);
    shaded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_flat_picture_faces_the_viewer() {
        let n = normals(&RgbaImage::from_pixel(8, 8, Rgba([90, 90, 90, 255])), 4.0, 0.0);
        assert!(n.pixels().all(|p| p.0 == [128, 128, 255, 255]), "{:?}", n.get_pixel(0, 0));
    }

    #[test]
    fn a_ramp_tilts_towards_its_low_side() {
        let ramp =
            RgbaImage::from_fn(16, 4, |x, _| Rgba([(x * 16) as u8, (x * 16) as u8, (x * 16) as u8, 255]));
        let n = normals(&ramp, 4.0, 0.0);
        let [r, g, b, _] = n.get_pixel(8, 2).0;
        assert!(r < 120 && g == 128 && b < 255, "{r} {g} {b}");
        let up =
            RgbaImage::from_fn(4, 16, |_, y| Rgba([(y * 16) as u8, (y * 16) as u8, (y * 16) as u8, 255]));
        assert!(normals(&up, 4.0, 0.0).get_pixel(2, 8).0[1] > 136);
    }

    #[test]
    fn blur_softens_the_edges_and_strength_scales_them() {
        let step = RgbaImage::from_fn(16, 4, |x, _| Rgba([if x < 8 { 0 } else { 255 }; 4]));
        let sharp = normals(&step, 2.0, 0.0).get_pixel(8, 2).0[0];
        let soft = normals(&step, 2.0, 3.0).get_pixel(8, 2).0[0];
        let weak = normals(&step, 0.2, 0.0).get_pixel(8, 2).0[0];
        assert!(sharp < soft && soft < 128 && weak > sharp, "{sharp} {soft} {weak}");
    }

    #[test]
    fn previews_shade_by_the_light() {
        let ball = sphere(32, [-1.0, 1.0, 1.0]);
        assert!(ball.get_pixel(8, 8).0[0] > ball.get_pixel(24, 24).0[0]);
        assert_eq!(ball.get_pixel(0, 0).0[3], 0);
        let flat = relief(&RgbaImage::from_pixel(2, 2, Rgba([128, 128, 255, 255])), [0.0, 0.0, 1.0]);
        assert_eq!(flat.get_pixel(0, 0).0[0], 255);
    }
}
