use image::imageops::FilterType;
use image::{ImageReader, Limits, RgbaImage};
use reqwest::Url;
use std::io::Cursor;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use wprs_render::Pixels;
use wprs_scene::{COVER_SIDE, FALLBACK, PALETTE_SIDE, palette};

pub(crate) const MAX_FILE: u64 = 16 << 20;
pub(crate) const MAX_REMOTE: usize = 4 << 20;
pub(crate) const PATIENCE: Duration = Duration::from_secs(5);
const MAX_SIDE: u32 = 8192;
const AGENT: &str = concat!("wallpaper-rs/", env!("CARGO_PKG_VERSION"));

static SERIAL: AtomicU64 = AtomicU64::new(1);

pub(crate) struct Cover {
    pub(crate) serial: u64,
    pub(crate) pixels: Arc<Pixels>,
    pub(crate) colors: [[f32; 4]; 5],
    pub(crate) digest: u64,
}

impl std::fmt::Debug for Cover {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Cover({})", self.serial)
    }
}

pub(crate) fn colors(image: &RgbaImage) -> Option<[[f32; 4]; 5]> {
    let small = image::imageops::resize(image, PALETTE_SIDE, PALETTE_SIDE, FilterType::Triangle);
    palette(small.as_raw())
}

pub(crate) fn reduced(image: RgbaImage) -> RgbaImage {
    let (w, h) = image.dimensions();
    if w <= COVER_SIDE && h <= COVER_SIDE {
        return image;
    }
    let scale = f64::from(COVER_SIDE) / f64::from(w.max(h));
    let size = |v: u32| ((f64::from(v) * scale).round() as u32).clamp(1, COVER_SIDE);
    image::imageops::resize(&image, size(w), size(h), FilterType::Triangle)
}

pub(crate) fn decode(bytes: &[u8]) -> Option<Arc<Cover>> {
    let digest = std::hash::BuildHasher::hash_one(
        &std::hash::BuildHasherDefault::<std::hash::DefaultHasher>::default(),
        bytes,
    );
    let mut reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format().ok()?;
    let mut limits = Limits::default();
    (limits.max_image_width, limits.max_image_height) = (Some(MAX_SIDE), Some(MAX_SIDE));
    limits.max_alloc = Some(512 << 20);
    reader.limits(limits);
    let image = reduced(reader.decode().ok()?.into_rgba8());
    let colors = colors(&image).unwrap_or(FALLBACK);
    let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
    Some(Arc::new(Cover { serial, pixels: Pixels::new(image), colors, digest }))
}

pub(crate) fn local(url: &Url) -> Option<Vec<u8>> {
    let path = url.to_file_path().ok()?;
    let size = std::fs::metadata(&path).ok().filter(|m| m.is_file())?.len();
    (size <= MAX_FILE).then(|| std::fs::read(&path).ok()).flatten()
}

async fn remote(url: Url) -> Option<Vec<u8>> {
    let client =
        reqwest::Client::builder().user_agent(AGENT).referer(false).timeout(PATIENCE).build().ok()?;
    let mut response = client.get(url).send().await.ok()?.error_for_status().ok()?;
    if response.content_length().is_some_and(|n| n > MAX_REMOTE as u64) {
        return None;
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.ok()? {
        bytes.extend_from_slice(&chunk);
        if bytes.len() > MAX_REMOTE {
            return None;
        }
    }
    Some(bytes)
}

pub(crate) async fn fetch(address: &str, remote_on: bool) -> Option<Arc<Cover>> {
    let url = Url::parse(address).ok()?;
    let bytes = match url.scheme() {
        "file" => tokio::task::spawn_blocking(move || local(&url)).await.ok()??,
        "http" | "https" if remote_on => tokio::time::timeout(PATIENCE, remote(url)).await.ok()??,
        _ => return None,
    };
    tokio::task::spawn_blocking(move || decode(&bytes)).await.ok()?
}
