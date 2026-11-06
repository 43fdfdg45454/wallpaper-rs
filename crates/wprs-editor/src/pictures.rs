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
    raw(path).map(|picture| wprs_ktx2::light::shown(picture, false))
}

pub fn raw(path: impl AsRef<Path>) -> ImageResult<DynamicImage> {
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
    Ok(wprs_ktx2::info(&std::fs::read(path)?).map_err(failure)?.picture(0))
}

pub fn glance(path: &Path) -> Option<wprs_ktx2::kv::Glance> {
    use std::io::Read;
    let mut head = Vec::new();
    std::fs::File::open(path).ok().filter(|_| ktx2(path))?.take(4096).read_to_end(&mut head).ok()?;
    wprs_ktx2::kv::glance(&head)
}

fn packed_like(like: &Path, image: &image::RgbaImage) -> Option<Vec<u8>> {
    let bytes = std::fs::read(like).ok().filter(|_| ktx2(like))?;
    let info = wprs_ktx2::info(&bytes).ok()?;
    let codec = if info.codec == "etc1s" { wprs_ktx2::Codec::Etc1s } else { wprs_ktx2::Codec::Uastc };
    let quality = wprs_ktx2::quality(&bytes).unwrap_or(crate::packing::QUALITY);
    let options = wprs_ktx2::Options { codec, quality, srgb: false, mips: info.levels > 1, repeat: false };
    wprs_ktx2::encode(image, &options).ok()
}

pub fn store(
    dir: &Path,
    (folder, name): (&str, &str),
    image: &image::RgbaImage,
    like: Option<&Path>,
) -> anyhow::Result<String> {
    let packed = like.and_then(|l| packed_like(l, image));
    let ext = if packed.is_some() { "ktx2" } else { "png" };
    let path = (1..)
        .map(|n| format!("{folder}/{name}-{n}.{ext}"))
        .find(|p| !dir.join(p).exists())
        .unwrap_or_default();
    std::fs::create_dir_all(dir.join(folder))?;
    match packed {
        Some(bytes) => std::fs::write(dir.join(&path), bytes)?,
        None => image.save(dir.join(&path))?,
    }
    Ok(path)
}
