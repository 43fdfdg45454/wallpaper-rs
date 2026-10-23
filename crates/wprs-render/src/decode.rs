use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageReader, RgbaImage};
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use wprs_core::{Edit, Fit, Place, UvRect};

const MAX_EDGE: u32 = 8192;
const MAX_PIXELS: f64 = 7680.0 * 4320.0;

pub fn decode_image(path: &Path, max_edge: u32) -> image::ImageResult<RgbaImage> {
    let mut decoder = ImageReader::open(path)?.with_guessed_format()?.into_decoder()?;
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut image = shrink(DynamicImage::from_decoder(decoder)?, max_edge);
    image.apply_orientation(orientation);
    Ok(image.into_rgba8())
}

pub fn image_size(path: &Path) -> image::ImageResult<(u32, u32)> {
    let mut decoder = ImageReader::open(path)?.with_guessed_format()?.into_decoder()?;
    let (w, h) = decoder.dimensions();
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let turned = matches!(
        orientation,
        Orientation::Rotate90
            | Orientation::Rotate270
            | Orientation::Rotate90FlipH
            | Orientation::Rotate270FlipH
    );
    Ok(if turned { (h, w) } else { (w, h) })
}

fn limit((w, h): (u32, u32), max_edge: u32) -> f64 {
    let (w, h) = (f64::from(w.max(1)), f64::from(h.max(1)));
    (f64::from(max_edge.min(MAX_EDGE)) / w.max(h)).min((MAX_PIXELS / (w * h)).sqrt())
}

pub(crate) fn shown(frame: (u32, u32), uv: UvRect, target: (u32, u32)) -> f32 {
    let shown = [0, 1].map(|i| [target.0, target.1][i] as f32 / uv.scale[i].abs().max(1e-6));
    (shown[0] / frame.0.max(1) as f32).max(shown[1] / frame.1.max(1) as f32)
}

pub fn needed_edge(
    path: &Path,
    edit: Option<&Edit>,
    place: &Place,
    (fit, target): (Fit, (u32, u32)),
    max_edge: u32,
) -> u32 {
    let Ok(image) = image_size(path) else { return max_edge };
    let frame = edit.map_or(image, |e| e.geometry.frame(image));
    let factor = f64::from(shown(frame, place.uv(fit, frame, target), target)).min(1.0);
    (f64::from(image.0.max(image.1)) * factor.min(limit(image, max_edge))).ceil() as u32
}

fn shrink(image: DynamicImage, max_edge: u32) -> DynamicImage {
    let (width, height) = (f64::from(image.width()), f64::from(image.height()));
    let scale = limit((image.width(), image.height()), max_edge);
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
    uv: UvRect,
    target: (u32, u32),
    max_edge: u32,
) -> Cow<'_, RgbaImage> {
    scaled(img, shown(img.dimensions(), uv, target), max_edge)
}

pub(crate) fn scaled(img: &RgbaImage, factor: f32, max_edge: u32) -> Cow<'_, RgbaImage> {
    let (iw, ih) = img.dimensions();
    let edge = iw.max(ih) as f32;
    let factor = factor.min(1.0);
    let factor = if edge * factor > max_edge as f32 { max_edge as f32 / edge } else { factor };
    if factor >= 0.5 && iw <= max_edge && ih <= max_edge {
        return Cow::Borrowed(img);
    }
    let size = |v: u32| ((v as f32 * factor).round() as u32).max(1);
    Cow::Owned(image::imageops::resize(img, size(iw), size(ih), FilterType::Triangle))
}

pub fn thumbnails(
    mut image: DynamicImage,
    thumbs: &[(u32, PathBuf)],
    written: &mut Vec<u32>,
) -> image::ImageResult<()> {
    let mut thumbs = thumbs.to_vec();
    thumbs.sort_by_key(|(size, _)| std::cmp::Reverse(*size));
    for (size, dest) in thumbs {
        if image.width().max(image.height()) > size {
            image = image.resize(size, size, FilterType::Triangle);
        }
        let dir = dest.parent().unwrap_or(Path::new("."));
        let mut file = tempfile::Builder::new().prefix(".tmp-").tempfile_in(dir)?;
        image.to_rgb8().write_with_encoder(JpegEncoder::new_with_quality(&mut file, 85))?;
        file.persist(&dest).map_err(|e| e.error)?;
        written.push(size);
    }
    Ok(())
}
