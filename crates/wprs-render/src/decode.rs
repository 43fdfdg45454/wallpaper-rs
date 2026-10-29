use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::metadata::Orientation;
use image::{DynamicImage, GrayImage, ImageDecoder, ImageReader, Limits, RgbImage, RgbaImage};
use jpeg_decoder::{CodingProcess, PixelFormat};
use std::borrow::Cow;
use std::io::{BufRead, BufReader, Seek};
use std::path::{Path, PathBuf};
use wprs_core::{Edit, Fit, Place, UvRect};

const MAX_EDGE: u32 = 8192;
const MAX_PIXELS: f64 = 7680.0 * 4320.0;
const BUDGET: u64 = 512 << 20;
const MAX_SIDE: u32 = 1 << 15;
const MAX_ALLOC: u64 = 1 << 30;
const MAX_BASELINE: u64 = 300_000_000;

pub fn picture_limits() -> Limits {
    let mut limits = Limits::default();
    (limits.max_image_width, limits.max_image_height) = (Some(MAX_SIDE), Some(MAX_SIDE));
    limits.max_alloc = Some(MAX_ALLOC);
    limits
}

pub fn decode_image(path: &Path, max_edge: u32) -> image::ImageResult<RgbaImage> {
    if let Some((image, _)) = decode_reduced(path, max_edge) {
        return Ok(image.into_rgba8());
    }
    let (image, orientation) = decode_unoriented(ImageReader::open(path)?, picture_limits())?;
    let mut image = shrink(image, max_edge);
    image.apply_orientation(orientation);
    Ok(image.into_rgba8())
}

pub fn decode_unoriented<R: BufRead + Seek>(
    reader: ImageReader<R>,
    mut limits: Limits,
) -> image::ImageResult<(DynamicImage, Orientation)> {
    let mut reader = reader.with_guessed_format()?;
    reader.limits(limits.clone());
    let mut decoder = reader.into_decoder()?;
    limits.reserve(decoder.total_bytes())?;
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    Ok((DynamicImage::from_decoder(decoder)?, orientation))
}

pub fn decode_limited<R: BufRead + Seek>(
    reader: ImageReader<R>,
    limits: Limits,
) -> image::ImageResult<DynamicImage> {
    let (mut image, orientation) = decode_unoriented(reader, limits)?;
    image.apply_orientation(orientation);
    Ok(image)
}

pub fn decode_reduced(path: &Path, max_edge: u32) -> Option<(DynamicImage, (u32, u32))> {
    let mut decoder = jpeg_decoder::Decoder::new(BufReader::new(std::fs::File::open(path).ok()?));
    decoder.read_info().ok()?;
    let info = decoder.info()?;
    let (width, height) = (u32::from(info.width), u32::from(info.height));
    let (scale, progressive) = (limit((width, height), max_edge), info.coding_process);
    let pixels = u64::from(width) * u64::from(height);
    let large = match progressive {
        CodingProcess::DctProgressive => pixels * 6 > BUDGET,
        _ => pixels > MAX_BASELINE,
    } || width.max(height) > MAX_SIDE;
    if scale > 0.5 || large || !matches!(info.pixel_format, PixelFormat::RGB24 | PixelFormat::L8) {
        return None;
    }
    decoder.set_max_decoding_buffer_size(BUDGET as usize);
    let wanted = |v: u16| ((f64::from(v) * scale).ceil() as u16).max(1);
    let (w, h) = decoder.scale(wanted(info.width), wanted(info.height)).ok()?;
    let (pixels, (w, h)) = (decoder.decode().ok()?, (u32::from(w), u32::from(h)));
    let image = match info.pixel_format {
        PixelFormat::L8 => DynamicImage::from(GrayImage::from_raw(w, h, pixels)?),
        _ => DynamicImage::from(RgbImage::from_raw(w, h, pixels)?),
    };
    let orientation = decoder.exif_data().and_then(Orientation::from_exif_chunk);
    let orientation = orientation.unwrap_or(Orientation::NoTransforms);
    let mut image = shrink(image, max_edge);
    image.apply_orientation(orientation);
    Some((image, if turned(orientation) { (height, width) } else { (width, height) }))
}

fn turned(orientation: Orientation) -> bool {
    use Orientation::{Rotate90, Rotate90FlipH, Rotate270, Rotate270FlipH};
    matches!(orientation, Rotate90 | Rotate270 | Rotate90FlipH | Rotate270FlipH)
}

pub fn image_size(path: &Path) -> image::ImageResult<(u32, u32)> {
    let mut decoder = ImageReader::open(path)?.with_guessed_format()?.into_decoder()?;
    let (w, h) = decoder.dimensions();
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    Ok(if turned(orientation) { (h, w) } else { (w, h) })
}

fn limit((w, h): (u32, u32), max_edge: u32) -> f64 {
    let (w, h) = (f64::from(w.max(1)), f64::from(h.max(1)));
    (f64::from(max_edge.min(MAX_EDGE)) / w.max(h)).min((MAX_PIXELS / (w * h)).sqrt())
}

pub(crate) fn shown(frame: (u32, u32), uv: UvRect, target: (u32, u32)) -> f32 {
    let shown = [0, 1].map(|i| [target.0, target.1][i] as f32 / uv.scale[i].abs().max(1e-6));
    (shown[0] / frame.0.max(1) as f32).max(shown[1] / frame.1.max(1) as f32)
}

type Shown<'a> = (Option<&'a Edit>, &'a Place, (Fit, (u32, u32)));

fn edge(image: (u32, u32), (edit, place, (fit, target)): Shown<'_>, max_edge: u32) -> u32 {
    let frame = edit.map_or(image, |e| e.geometry.frame(image));
    let factor = f64::from(shown(frame, place.uv(fit, frame, target), target)).min(1.0);
    (f64::from(image.0.max(image.1)) * factor.min(limit(image, max_edge))).ceil() as u32
}

pub fn needed_edge(
    path: &Path,
    edit: Option<&Edit>,
    place: &Place,
    target: (Fit, (u32, u32)),
    max_edge: u32,
) -> u32 {
    image_size(path).map_or(max_edge, |image| edge(image, (edit, place, target), max_edge))
}

pub fn decoded_size(
    path: &Path,
    edit: Option<&Edit>,
    place: &Place,
    target: (Fit, (u32, u32)),
    max_edge: u32,
) -> Option<(u32, u32)> {
    let image = image_size(path).ok()?;
    Some(fitted(image, edge(image, (edit, place, target), max_edge)))
}

fn fitted((width, height): (u32, u32), max_edge: u32) -> (u32, u32) {
    let scale = limit((width, height), max_edge);
    let size = |v: u32| ((f64::from(v) * scale).round() as u32).max(1);
    if scale >= 1.0 { (width, height) } else { (size(width), size(height)) }
}

fn shrink(image: DynamicImage, max_edge: u32) -> DynamicImage {
    let (w, h) = fitted((image.width(), image.height()), max_edge);
    match limit((image.width(), image.height()), max_edge) {
        scale if scale >= 1.0 => image,
        scale if scale <= 0.5 => image.thumbnail_exact(w, h),
        _ => image.resize_exact(w, h, FilterType::Triangle),
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

pub(crate) fn scaled_size((iw, ih): (u32, u32), factor: f32, max_edge: u32) -> (u32, u32) {
    let edge = iw.max(ih) as f32;
    let factor = factor.min(1.0);
    let factor = if edge * factor > max_edge as f32 { max_edge as f32 / edge } else { factor };
    if factor >= 0.5 && iw <= max_edge && ih <= max_edge {
        return (iw, ih);
    }
    let size = |v: u32| ((v as f32 * factor).round() as u32).max(1);
    (size(iw), size(ih))
}

pub(crate) fn scaled(img: &RgbaImage, factor: f32, max_edge: u32) -> Cow<'_, RgbaImage> {
    match scaled_size(img.dimensions(), factor, max_edge) {
        size if size == img.dimensions() => Cow::Borrowed(img),
        (w, h) => Cow::Owned(image::imageops::resize(img, w, h, FilterType::Triangle)),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn large_jpegs_are_decoded_at_a_reduced_scale_with_the_same_result_size() {
        let dir = tempfile::tempdir().unwrap();
        let (jpeg, png) = (dir.path().join("wide.jpg"), dir.path().join("wide.png"));
        let image = RgbImage::from_fn(1600, 1200, |x, _| image::Rgb([(x % 256) as u8, 128, 0]));
        image.save(&jpeg).unwrap();
        image.save(&png).unwrap();
        let (reduced, full) = decode_reduced(&jpeg, 300).expect("a reduced decode");
        assert_eq!((reduced.width(), reduced.height(), full), (300, 225, (1600, 1200)));
        assert!(decode_reduced(&jpeg, 1000).is_none() && decode_reduced(&png, 300).is_none());
        assert_eq!(
            decode_image(&jpeg, 300).unwrap().dimensions(),
            decode_image(&png, 300).unwrap().dimensions()
        );
    }

    fn huge_jpeg(dir: &Path, name: &str, progressive: bool, (width, height): (u16, u16)) -> PathBuf {
        let mut bytes = Vec::new();
        let image = RgbImage::from_pixel(16, 16, image::Rgb([9, 9, 9]));
        image.write_with_encoder(JpegEncoder::new(&mut bytes)).unwrap();
        let at = bytes.windows(2).position(|m| m == [0xff, 0xc0]).expect("a baseline frame header");
        bytes[at + 1] = if progressive { 0xc2 } else { 0xc0 };
        bytes[at + 5..at + 9].copy_from_slice(&[height.to_be_bytes(), width.to_be_bytes()].concat());
        let path = dir.join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn tiny_jpegs_that_claim_huge_sizes_are_refused_at_once() {
        let dir = tempfile::tempdir().unwrap();
        for (progressive, size) in
            [(false, (60000, 60000)), (true, (60000, 60000)), (true, (12000, 10000)), (false, (40000, 3000))]
        {
            let path = huge_jpeg(dir.path(), "huge.jpg", progressive, size);
            let start = std::time::Instant::now();
            assert!(decode_reduced(&path, 3840).is_none(), "{size:?} was decoded at a reduced scale");
            assert!(size.0 < 60000 || decode_image(&path, 3840).is_err(), "{size:?} was decoded");
            assert!(
                start.elapsed() < std::time::Duration::from_secs(2),
                "{size:?} took {:?}",
                start.elapsed()
            );
        }
        let same = huge_jpeg(dir.path(), "same.jpg", false, (16, 16));
        assert_eq!(decode_image(&same, 3840).unwrap().dimensions(), (16, 16));
    }

    #[test]
    fn large_baseline_photos_are_decoded_at_a_reduced_scale() {
        let dir = tempfile::tempdir().unwrap();
        let photo = huge_jpeg(dir.path(), "photo.jpg", false, (16320, 12240));
        let start = std::time::Instant::now();
        let (reduced, full) = decode_reduced(&photo, 2040).expect("a reduced decode");
        let bound = std::time::Duration::from_secs(if cfg!(debug_assertions) { 20 } else { 2 });
        assert!(start.elapsed() < bound, "it took {:?}", start.elapsed());
        assert_eq!(((reduced.width(), reduced.height()), full), ((2040, 1530), (16320, 12240)));
        assert_eq!(decode_image(&photo, 3840).unwrap().width(), 3840);
    }
}
