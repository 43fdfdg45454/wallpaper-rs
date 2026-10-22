use image::imageops::FilterType;
use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageReader, RgbaImage};
use std::borrow::Cow;
use std::path::Path;
use wprs_core::{Fit, UvRect};

const MAX_EDGE: u32 = 8192;
const MAX_PIXELS: f64 = 7680.0 * 4320.0;

pub fn decode_image(path: &Path, max_edge: u32) -> image::ImageResult<RgbaImage> {
    let mut decoder = ImageReader::open(path)?.with_guessed_format()?.into_decoder()?;
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut image = shrink(DynamicImage::from_decoder(decoder)?, max_edge.min(MAX_EDGE));
    image.apply_orientation(orientation);
    Ok(image.into_rgba8())
}

fn shrink(image: DynamicImage, max_edge: u32) -> DynamicImage {
    let (width, height) = (f64::from(image.width()), f64::from(image.height()));
    let scale = (f64::from(max_edge) / width.max(height)).min((MAX_PIXELS / (width * height)).sqrt());
    if scale >= 1.0 {
        return image;
    }
    let size = |v: f64| ((v * scale).round() as u32).max(1);
    let (w, h) = (size(width), size(height));
    match scale <= 0.5 {
        true => image.thumbnail_exact(w, h),
        false => image.resize_exact(w, h, FilterType::Triangle),
    }
}

pub(crate) fn prepare_image(
    img: &RgbaImage,
    fit: Fit,
    target: (u32, u32),
    max_edge: u32,
) -> Cow<'_, RgbaImage> {
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
    Cow::Owned(image::imageops::resize(img, size(iw), size(ih), FilterType::Triangle))
}
