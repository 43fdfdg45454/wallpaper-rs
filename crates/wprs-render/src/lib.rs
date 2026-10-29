mod adapter;
#[cfg(test)]
mod allocations;
mod animation;
mod bind;
mod color;
mod decode;
mod develop;
mod device;
#[cfg(target_os = "linux")]
mod dmabuf;
mod draw;
mod export;
#[cfg(target_os = "linux")]
mod exported;
mod luma;
mod palette;
mod parallax;
mod readback;
mod scene;
mod shader;
mod stack;
mod surface;
mod texture;
mod yuv;

pub use animation::{MAX_ANIMATION, Pictures, animation};
pub use decode::{
    decode_image, decode_limited, decode_reduced, decode_unoriented, decoded_size, image_size, needed_edge,
    picture_limits, thumbnails,
};
pub use device::{Display, Renderer};
#[cfg(target_os = "linux")]
pub use dmabuf::{Dmabuf, Dmabufs};
pub use draw::Target;
#[cfg(target_os = "linux")]
pub use exported::Exported;
pub use luma::Meter;
pub use palette::{Palette, palette};
pub use parallax::{DepthMap, Relief};
pub use readback::TargetLimits;
pub use scene::{MAX_FONT, Pixels, SceneAssets, SceneStats, SceneView, font};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
pub use surface::{Presentation, Surface};
pub use texture::GpuImage;
use wprs_core::{Fit, UvRect};
pub use yuv::{Planes, Yuv};

const LOG_PERIOD: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, thiserror::Error)]
pub enum RenderError {
    #[error("no compatible GPU adapter found: {0}")]
    NoAdapter(String),
    #[error("cannot create GPU device: {0}")]
    Device(String),
    #[error("invalid target size {0}x{1}")]
    InvalidSize(u32, u32),
    #[error("GPU error: {0}")]
    Gpu(String),
    #[error("GPU readback failed: {0}")]
    Readback(String),
    #[error("the GPU stopped responding: {0}")]
    DeviceLost(String),
    #[error("cannot present to the surface: {0}")]
    Surface(String),
}

#[derive(Clone, Copy)]
pub struct Layer<'a> {
    pub image: Option<&'a GpuImage>,
    pub fit: Fit,
    pub background: [u8; 3],
    pub opacity: f32,
    pub uv: Option<UvRect>,
    pub relief: Option<Relief<'a>>,
}

pub fn log_allowed(last: &AtomicU64) -> bool {
    static START: OnceLock<Instant> = OnceLock::new();
    let period = LOG_PERIOD.as_millis() as u64;
    let now = START.get_or_init(Instant::now).elapsed().as_millis() as u64 + period;
    let previous = last.load(Ordering::Relaxed);
    now >= previous + period
        && last.compare_exchange(previous, now, Ordering::Relaxed, Ordering::Relaxed).is_ok()
}
