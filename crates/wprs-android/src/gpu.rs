use crate::settings::{self, BACKEND};
use ndk::native_window::NativeWindow;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex, PoisonError};
use wgpu::rwh::{AndroidDisplayHandle, AndroidNdkWindowHandle, RawDisplayHandle, RawWindowHandle};
use wprs_core::Fit;
use wprs_render::{GpuImage, Presentation, RenderError, Renderer, Surface, decode_image};

static SHARED: Mutex<Option<Gpu>> = Mutex::new(None);
static EPOCH: AtomicU64 = AtomicU64::new(0);
static LOST: AtomicBool = AtomicBool::new(false);
static GL_ONLY: AtomicBool = AtomicBool::new(false);
static TEXTURES: Mutex<Vec<(Key, Arc<GpuImage>)>> = Mutex::new(Vec::new());
static ADAPTER: Mutex<String> = Mutex::new(String::new());

const BLOCKED: &[&str] = &[];

#[derive(Clone)]
pub(crate) struct Gpu {
    pub(crate) renderer: Arc<Renderer>,
    pub(crate) epoch: u64,
    vulkan: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Key {
    pub(crate) epoch: u64,
    pub(crate) item: String,
    pub(crate) fit: Fit,
    pub(crate) size: (u32, u32),
}

impl Gpu {
    pub(crate) fn current(&self) -> bool {
        !LOST.load(Relaxed) && EPOCH.load(Relaxed) == self.epoch
    }

    pub(crate) fn texture(&self, key: Key, path: &Path) -> Option<Arc<GpuImage>> {
        let mut cache = TEXTURES.lock().unwrap_or_else(PoisonError::into_inner);
        cache.retain(|(cached, image)| cached.epoch == self.epoch && Arc::strong_count(image) > 1);
        if let Some((_, image)) = cache.iter().find(|(cached, _)| *cached == key) {
            return Some(image.clone());
        }
        let decoded = decode_image(path, self.renderer.max_edge())
            .inspect_err(|e| log::warn!("cannot read the wallpaper image: {e}"))
            .ok()?;
        let image = self.renderer.upload(&decoded, key.fit, key.size).inspect_err(|e| log::error!("{e}"));
        let image = Arc::new(image.ok()?);
        cache.push((key, image.clone()));
        Some(image)
    }
}

pub(crate) fn reset() {
    GL_ONLY.store(false, Relaxed);
    LOST.store(true, Relaxed);
}

pub(crate) fn adapter() -> String {
    ADAPTER.lock().unwrap_or_else(PoisonError::into_inner).clone()
}

pub(crate) fn trim() {
    TEXTURES.lock().unwrap_or_else(PoisonError::into_inner).retain(|(_, image)| Arc::strong_count(image) > 1);
}

fn lock() -> std::sync::MutexGuard<'static, Option<Gpu>> {
    SHARED.lock().unwrap_or_else(PoisonError::into_inner)
}

fn surface(instance: &wgpu::Instance, window: &NativeWindow) -> Result<wgpu::Surface<'static>, RenderError> {
    let target = wgpu::SurfaceTargetUnsafe::RawHandle {
        raw_display_handle: Some(RawDisplayHandle::Android(AndroidDisplayHandle::new())),
        raw_window_handle: RawWindowHandle::AndroidNdk(AndroidNdkWindowHandle::new(window.ptr().cast())),
    };
    unsafe { instance.create_surface_unsafe(target) }.map_err(|e| RenderError::Surface(e.to_string()))
}

fn driver(e: &RenderError) -> bool {
    !matches!(e, RenderError::InvalidSize(..))
}

fn configure(renderer: &Renderer, window: &NativeWindow, size: (u32, u32)) -> Result<Surface, RenderError> {
    renderer.configure_surface(surface(renderer.instance(), window)?, size, Presentation::MOBILE)
}

pub(crate) fn open(window: &NativeWindow, size: (u32, u32)) -> Result<(Gpu, Surface), RenderError> {
    let (mut shared, chosen) = (lock(), settings::chosen(BACKEND));
    if LOST.swap(false, Relaxed) {
        *shared = None;
    }
    if let Some(gpu) = shared.clone() {
        match configure(&gpu.renderer, window, size) {
            Ok(surface) => return Ok((gpu, surface)),
            Err(e) if gpu.vulkan && chosen == "auto" && driver(&e) && !GL_ONLY.swap(true, Relaxed) => {
                log::warn!("{e}, switching to OpenGL ES");
                *shared = None;
            }
            Err(e) => return Err(e),
        }
    }
    let mut failure = RenderError::NoAdapter("no backend".into());
    for backends in [wgpu::Backends::VULKAN, wgpu::Backends::GL] {
        let vulkan = backends == wgpu::Backends::VULKAN;
        let skip = match chosen.as_str() {
            "gl" => vulkan,
            "vulkan" => !vulkan,
            _ => vulkan && GL_ONLY.load(Relaxed),
        };
        if skip {
            continue;
        }
        match create(backends, window, size) {
            Ok((renderer, surface)) => {
                log::debug!("rendering with {}", renderer.adapter());
                *ADAPTER.lock().unwrap_or_else(PoisonError::into_inner) = renderer.adapter().to_owned();
                let epoch = EPOCH.fetch_add(1, Relaxed) + 1;
                renderer.on_device_lost(move |reason| {
                    log::error!("the GPU device was lost: {reason}");
                    LOST.fetch_or(EPOCH.load(Relaxed) == epoch, Relaxed);
                });
                let gpu = Gpu { renderer: Arc::new(renderer), epoch, vulkan };
                *shared = Some(gpu.clone());
                return Ok((gpu, surface));
            }
            Err(e) => {
                log::warn!("{e}");
                GL_ONLY.fetch_or(vulkan && chosen == "auto" && driver(&e), Relaxed);
                failure = e;
            }
        }
    }
    Err(failure)
}

fn create(
    backends: wgpu::Backends,
    window: &NativeWindow,
    size: (u32, u32),
) -> Result<(Renderer, Surface), RenderError> {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = backends;
    let instance = wgpu::Instance::new(desc);
    let surface = surface(&instance, window)?;
    let options = wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        compatible_surface: Some(&surface),
        ..Default::default()
    };
    let adapter = pollster::block_on(instance.request_adapter(&options))
        .map_err(|e| RenderError::NoAdapter(format!("{backends:?}: {e}")))?;
    let info = adapter.get_info();
    if BLOCKED.iter().any(|name| info.name.contains(name)) {
        return Err(RenderError::NoAdapter(format!("{} ({:?}) is blocked", info.name, info.backend)));
    }
    let renderer = Renderer::with_adapter(instance, adapter)?;
    let surface = renderer.configure_surface(surface, size, Presentation::MOBILE)?;
    Ok((renderer, surface))
}
