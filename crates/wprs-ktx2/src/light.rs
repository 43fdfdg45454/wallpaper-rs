use image::{DynamicImage, Rgba32FImage, RgbaImage};

pub fn encoded(v: f32) -> u8 {
    let v = v.clamp(0.0, 1.0);
    let s = if v <= 0.003_130_8 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 };
    (s * 255.0).round() as u8
}

pub fn decoded(code: u8) -> f32 {
    let s = f32::from(code) / 255.0;
    if s <= 0.040_45 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
}

pub(crate) fn alpha(v: f32) -> u8 {
    let v = v.clamp(0.0, 1.0);
    (v * 255.0).round() as u8
}

pub fn bright(picture: &DynamicImage) -> bool {
    matches!(picture, DynamicImage::ImageRgb32F(_) | DynamicImage::ImageRgba32F(_))
}

pub fn eight(picture: &DynamicImage, linear: bool) -> RgbaImage {
    if linear || !bright(picture) {
        return picture.to_rgba8();
    }
    let wide = picture.to_rgba32f();
    let texel = |p: &[f32; 4]| [encoded(p[0]), encoded(p[1]), encoded(p[2]), alpha(p[3])];
    let data = wide.as_raw().as_chunks::<4>().0.iter().flat_map(texel).collect();
    RgbaImage::from_raw(wide.width(), wide.height(), data).unwrap_or_default()
}

pub fn shown(picture: DynamicImage, linear: bool) -> DynamicImage {
    match linear || !bright(&picture) {
        true => picture,
        false => DynamicImage::ImageRgba8(eight(&picture, false)),
    }
}

pub fn wide(picture: &DynamicImage, linear: bool) -> Rgba32FImage {
    if linear || bright(picture) {
        return picture.to_rgba32f();
    }
    let plain = picture.to_rgba8();
    let texel = |p: &[u8; 4]| [decoded(p[0]), decoded(p[1]), decoded(p[2]), f32::from(p[3]) / 255.0];
    let data = plain.as_raw().as_chunks::<4>().0.iter().flat_map(texel).collect();
    Rgba32FImage::from_raw(plain.width(), plain.height(), data).unwrap_or_default()
}
