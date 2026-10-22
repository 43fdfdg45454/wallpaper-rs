use crate::{Error, gpu};
use ndk::native_window::NativeWindow;
use std::fs::File;
use std::os::fd::BorrowedFd;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::{Mutex, OnceLock, PoisonError};
use wprs_core::Fit;
use wprs_render::{GpuImage, Layer, Renderer, Surface, decode_image, image_size};

const IMAGE: &str = "wallpaper";
const BACKGROUND: [u8; 3] = [24, 24, 28];

static FILES: OnceLock<PathBuf> = OnceLock::new();
static GENERATION: AtomicU64 = AtomicU64::new(1);
static SHOWN: Mutex<Option<(Loaded, GpuImage)>> = Mutex::new(None);

type Loaded = (u64, (u32, u32));

pub(crate) struct Engine {
    surface: Surface,
    _window: NativeWindow,
    renderer: &'static Renderer,
    size: (u32, u32),
    image: Option<GpuImage>,
    loaded: Loaded,
    dirty: bool,
}

fn size(width: i32, height: i32) -> (u32, u32) {
    (width.max(1).unsigned_abs(), height.max(1).unsigned_abs())
}

impl Engine {
    pub(crate) fn open(window: NativeWindow) -> Result<Self, Error> {
        let size = size(window.width(), window.height());
        let (renderer, surface) = gpu::open(&window, size)?;
        Ok(Self { surface, _window: window, renderer, size, image: None, loaded: (0, size), dirty: true })
    }

    pub(crate) fn resize(&mut self, width: i32, height: i32) {
        let size = size(width, height);
        self.dirty |= size != self.size;
        self.size = size;
    }

    pub(crate) fn draw(&mut self, force: bool) -> Result<bool, Error> {
        self.dirty |= force;
        let wanted = (GENERATION.load(Relaxed), self.size);
        if self.loaded != wanted {
            (self.image, self.loaded, self.dirty) = (texture(self.renderer, wanted), wanted, true);
        }
        if !self.dirty {
            return Ok(false);
        }
        let layer = Layer {
            image: self.image.as_ref(),
            fit: Fit::Fill,
            background: BACKGROUND,
            opacity: 1.0,
            uv: None,
            relief: None,
        };
        self.dirty = !self.renderer.present_layers(&mut self.surface, self.size, &[layer], || {})?;
        Ok(self.dirty)
    }
}

fn texture(renderer: &Renderer, wanted: Loaded) -> Option<GpuImage> {
    let mut shown = SHOWN.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some((loaded, image)) = shown.as_ref()
        && *loaded == wanted
    {
        return Some(image.clone());
    }
    *shown = None;
    let path = FILES.get()?.join(IMAGE);
    let decoded = decode_image(&path, renderer.max_edge())
        .inspect_err(|e| log::warn!("cannot read the wallpaper image: {e}"))
        .ok()?;
    let image = renderer.upload(&decoded, Fit::Fill, wanted.1).inspect_err(|e| log::error!("{e}")).ok()?;
    *shown = Some((wanted, image.clone()));
    Some(image)
}

pub(crate) fn prepare(files: String) {
    let _ = FILES.set(files.into());
}

pub(crate) fn store(fd: i32) -> Result<bool, Error> {
    let dir = FILES.get().ok_or(Error::Unprepared)?;
    let mut source = File::from(unsafe { BorrowedFd::borrow_raw(fd) }.try_clone_to_owned()?);
    let mut copy = tempfile::NamedTempFile::new_in(dir)?;
    std::io::copy(&mut source, &mut copy)?;
    image_size(copy.path()).map_err(|e| Error::Image(e.to_string()))?;
    copy.as_file().sync_all()?;
    copy.persist(dir.join(IMAGE)).map_err(|e| e.error)?;
    GENERATION.fetch_add(1, Relaxed);
    Ok(true)
}
