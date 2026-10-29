use super::Image;
use crate::Renderer;
use crate::texture::{gpu_image, texture};
use image::RgbaImage;
use std::sync::{Arc, Mutex, PoisonError, Weak};
use wgpu::TextureFormat as Format;

type Reload = Box<dyn Fn() -> Option<RgbaImage> + Send + Sync>;

pub struct Pixels {
    size: (u32, u32),
    kept: Mutex<Option<Arc<RgbaImage>>>,
    reload: Option<Reload>,
}

type Entry = (Weak<Pixels>, [bool; 2], Weak<(crate::GpuImage, [f32; 2])>);

#[derive(Default)]
pub(crate) struct Uploads(Mutex<Vec<Entry>>);

impl Pixels {
    pub fn new(image: impl Into<Arc<RgbaImage>>) -> Arc<Pixels> {
        let image = image.into();
        Arc::new(Pixels { size: image.dimensions(), kept: Mutex::new(Some(image)), reload: None })
    }

    pub fn reloadable(
        image: impl Into<Arc<RgbaImage>>,
        reload: impl Fn() -> Option<RgbaImage> + Send + Sync + 'static,
    ) -> Arc<Pixels> {
        let image = image.into();
        let (size, kept) = (image.dimensions(), Mutex::new(Some(image)));
        Arc::new(Pixels { size, kept, reload: Some(Box::new(reload)) })
    }

    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    pub fn resident(&self) -> bool {
        self.kept.lock().unwrap_or_else(PoisonError::into_inner).is_some()
    }

    pub fn image(&self) -> Option<Arc<RgbaImage>> {
        let mut kept = self.kept.lock().unwrap_or_else(PoisonError::into_inner);
        if kept.is_none() {
            let fresh = self.reload.as_ref()?().filter(|i| i.dimensions() == self.size)?;
            *kept = Some(Arc::new(fresh));
        }
        kept.clone()
    }

    pub(crate) fn release(&self) {
        if self.reload.is_some() {
            *self.kept.lock().unwrap_or_else(PoisonError::into_inner) = None;
        }
    }
}

impl std::fmt::Debug for Pixels {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Pixels({}x{}, resident {})", self.size.0, self.size.1, self.resident())
    }
}

impl Renderer {
    pub(crate) fn uploaded(&self, pixels: &Arc<Pixels>, flags: [bool; 2]) -> Option<Image> {
        let mut list = self.uploads.0.lock().unwrap_or_else(PoisonError::into_inner);
        list.retain(|(p, _, image)| p.strong_count() > 0 && image.strong_count() > 0);
        let same =
            |(p, f, _): &&(Weak<Pixels>, [bool; 2], _)| p.as_ptr() == Arc::as_ptr(pixels) && *f == flags;
        list.iter().find(same).and_then(|(.., image)| image.upgrade())
    }

    pub(crate) fn keep(&self, pixels: &Arc<Pixels>, flags: [bool; 2], image: &Image) {
        let mut list = self.uploads.0.lock().unwrap_or_else(PoisonError::into_inner);
        list.push((Arc::downgrade(pixels), flags, Arc::downgrade(image)));
    }

    pub(crate) fn upload_pixels(
        &self,
        pixels: &Arc<Pixels>,
        linear: bool,
        mips: Option<bool>,
    ) -> Option<Image> {
        let mipped = || self.uploaded(pixels, [linear, true]).filter(|_| mips != Some(false));
        if let Some(found) = mipped().or_else(|| self.uploaded(pixels, [linear, false])) {
            return Some(found);
        }
        let image = pixels.image()?;
        let prepared = crate::decode::scaled(&image, 1.0, self.max_edge());
        let (w, h) = prepared.dimensions();
        let format = if linear { Format::Rgba8Unorm } else { Format::Rgba8UnormSrgb };
        let gpu = gpu_image(texture(self, prepared.as_raw(), (w, h), w * 4, format));
        let made = Arc::new((gpu, [image.width() as f32, image.height() as f32]));
        self.keep(pixels, [linear, false], &made);
        Some(made)
    }

    pub fn uploads_of(&self, pixels: &Arc<Pixels>) -> usize {
        let mut list = self.uploads.0.lock().unwrap_or_else(PoisonError::into_inner);
        list.retain(|(p, _, image)| p.strong_count() > 0 && image.strong_count() > 0);
        list.iter().filter(|(p, ..)| p.as_ptr() == Arc::as_ptr(pixels)).count()
    }
}
