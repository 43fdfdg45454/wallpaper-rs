use image::error::{DecodingError, ImageFormatHint};
use image::{DynamicImage, ImageError, ImageResult};
use std::path::Path;

fn ktx2(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("ktx2"))
}

fn failure(e: wprs_ktx2::Error) -> ImageError {
    ImageError::Decoding(DecodingError::new(ImageFormatHint::Name("KTX2".into()), e))
}

pub fn open(path: impl AsRef<Path>) -> ImageResult<DynamicImage> {
    let path = path.as_ref();
    if !ktx2(path) {
        return image::open(path);
    }
    let bytes = std::fs::read(path)?;
    wprs_ktx2::rgba(&bytes, wprs_ktx2::MAX_SIDE).map(DynamicImage::ImageRgba8).map_err(failure)
}

pub fn dimensions(path: impl AsRef<Path>) -> ImageResult<(u32, u32)> {
    let path = path.as_ref();
    if !ktx2(path) {
        return image::image_dimensions(path);
    }
    let info = wprs_ktx2::info(&std::fs::read(path)?).map_err(failure)?;
    Ok((info.width, info.height))
}
