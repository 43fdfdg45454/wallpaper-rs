use image::error::{DecodingError, ImageFormatHint};
use image::{DynamicImage, ImageError, ImageResult};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

const SHOWN_EDGE: u32 = 2048;
pub const SHOWN_KEPT: usize = 16;
const KEPT: usize = 8;
const KEPT_PIXELS: u64 = 2048 * 2048;

type Exact = (PathBuf, Option<SystemTime>, image::RgbaImage);

static EXACT: Mutex<Vec<Exact>> = Mutex::new(Vec::new());

fn stamp(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn keep(path: PathBuf, image: &image::RgbaImage) {
    if u64::from(image.width()) * u64::from(image.height()) > KEPT_PIXELS {
        return;
    }
    let mut kept = EXACT.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    kept.retain(|(p, ..)| *p != path);
    if kept.len() >= KEPT {
        kept.remove(0);
    }
    let when = stamp(&path);
    kept.push((path, when, image.clone()));
}

pub fn exact(path: &Path) -> Option<image::RgbaImage> {
    let kept = EXACT.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let found = kept.iter().find(|(p, ..)| p == path)?;
    (found.1.is_some() && found.1 == stamp(path)).then(|| found.2.clone())
}

fn ktx2(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("ktx2"))
}

fn failure(e: wprs_ktx2::Error) -> ImageError {
    ImageError::Decoding(DecodingError::new(ImageFormatHint::Name("KTX2".into()), e))
}

pub fn open(path: impl AsRef<Path>) -> ImageResult<DynamicImage> {
    raw(path).map(|picture| wprs_ktx2::light::shown(picture, false))
}

pub fn asset(dir: &Path, doc: &serde_json::Value, id: &str) -> ImageResult<DynamicImage> {
    let found = &doc["scene"]["assets"][id];
    let missing = || ImageError::IoError(std::io::Error::from(std::io::ErrorKind::NotFound));
    let path = dir.join(found["path"].as_str().ok_or_else(missing)?);
    if let Some(kept) = exact(&path) {
        return Ok(DynamicImage::ImageRgba8(kept));
    }
    raw(path).map(|picture| wprs_ktx2::light::shown(picture, found["linear"] == true))
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
        Some(bytes) => {
            std::fs::write(dir.join(&path), bytes)?;
            keep(dir.join(&path), image);
        }
        None => image.save(dir.join(&path))?,
    }
    Ok(path)
}

pub fn shown(path: impl AsRef<Path>, cache: &Path) -> ImageResult<PathBuf> {
    let path = path.as_ref();
    if !ktx2(path) {
        return Ok(path.to_path_buf());
    }
    let bytes = std::fs::read(path)?;
    let hash = wprs_format::sha256(&bytes[..]).map_err(|e| ImageError::IoError(std::io::Error::other(e)))?;
    let out = cache.join(format!("{hash}.png"));
    if out.is_file() {
        std::fs::File::options().append(true).open(&out)?.set_modified(std::time::SystemTime::now())?;
        return Ok(out);
    }
    std::fs::create_dir_all(cache)?;
    let part = cache.join(format!("{hash}.part"));
    let image = wprs_ktx2::rgba(&bytes, SHOWN_EDGE).map_err(failure)?;
    image.save_with_format(&part, image::ImageFormat::Png)?;
    std::fs::rename(&part, &out)?;
    pruned(cache);
    Ok(out)
}

fn pruned(cache: &Path) {
    let Ok(list) = std::fs::read_dir(cache) else { return };
    let mut kept: Vec<(std::time::SystemTime, PathBuf)> = list
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "png"))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    kept.sort_by_key(|a| std::cmp::Reverse(a.0));
    kept.iter().skip(SHOWN_KEPT).for_each(|(_, p)| drop(std::fs::remove_file(p)));
}
