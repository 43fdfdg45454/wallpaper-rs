use image::error::{DecodingError, ImageFormatHint};
use image::{DynamicImage, ImageError, ImageResult};
use std::path::{Path, PathBuf};

const SHOWN_EDGE: u32 = 2048;

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

pub fn shown(path: impl AsRef<Path>, cache: &Path) -> ImageResult<PathBuf> {
    let path = path.as_ref();
    if !ktx2(path) {
        return Ok(path.to_path_buf());
    }
    let bytes = std::fs::read(path)?;
    let hash = wprs_format::sha256(&bytes[..]).map_err(|e| ImageError::IoError(std::io::Error::other(e)))?;
    let out = cache.join(format!("{hash}.png"));
    if !out.is_file() {
        std::fs::create_dir_all(cache)?;
        let part = cache.join(format!("{hash}.part"));
        let image = wprs_ktx2::rgba(&bytes, SHOWN_EDGE).map_err(failure)?;
        image.save_with_format(&part, image::ImageFormat::Png)?;
        std::fs::rename(&part, &out)?;
    }
    Ok(out)
}
