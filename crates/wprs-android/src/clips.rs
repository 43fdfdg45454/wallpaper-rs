use crate::Error;
use crate::said;
use ndk::media::image_reader::{AcquireResult, Image, ImageFormat, ImageReader};
use ndk::native_window::NativeWindow;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::{Mutex, PoisonError};
use wprs_render::{GpuImage, Planes, Renderer, Yuv};

const EDGE: i32 = 1920;
const IMAGES: i32 = 3;

static SHOWN: AtomicU64 = AtomicU64::new(0);
static FAILED: AtomicU64 = AtomicU64::new(0);
static LAST: Mutex<String> = Mutex::new(String::new());

struct Clip {
    path: PathBuf,
    reader: ImageReader,
    yuv: Option<Yuv>,
    chroma: Vec<u8>,
    epoch: u64,
    warned: bool,
}

#[derive(Default)]
pub(crate) struct Clips(Vec<Clip>);

pub(crate) fn report() -> serde_json::Value {
    let last = LAST.lock().unwrap_or_else(PoisonError::into_inner).clone();
    serde_json::json!({ "frames": SHOWN.load(Relaxed), "failed": FAILED.load(Relaxed), "error": last })
}

fn failed(e: &Error, warned: &mut bool) {
    FAILED.fetch_add(1, Relaxed);
    if !std::mem::replace(warned, true) {
        log::warn!("{e}");
        *LAST.lock().unwrap_or_else(PoisonError::into_inner) = e.to_string();
    }
}

fn media(e: ndk::media_error::MediaError) -> Error {
    said("android.video_frames", "video frames: %1", &[&e])
}

fn planes<'a>(image: &'a Image, chroma: &'a mut Vec<u8>) -> Result<Planes<'a>, Error> {
    let crop = image.crop_rect().map_err(media)?;
    let (left, top) = (crop.left.max(0) as usize & !1, crop.top.max(0) as usize & !1);
    let (w, h) =
        ((crop.right - crop.left).max(0) as usize & !1, (crop.bottom - crop.top).max(0) as usize & !1);
    let stride = |i| image.plane_row_stride(i).map(|s| s.max(0) as usize).map_err(media);
    let (luma, row) = (image.plane_data(0).map_err(media)?, stride(0)?);
    let (u, v, row_uv) =
        (image.plane_data(1).map_err(media)?, image.plane_data(2).map_err(media)?, stride(1)?);
    let step = image.plane_pixel_stride(1).map_err(media)?.max(1) as usize;
    chroma.clear();
    for y in (top / 2)..(top + h) / 2 {
        let at = |x: usize| y * row_uv + x * step;
        let pairs = (left / 2..(left + w) / 2).map(|x| [u.get(at(x)), v.get(at(x))]);
        chroma.extend(pairs.flat_map(|[u, v]| [u.copied().unwrap_or(128), v.copied().unwrap_or(128)]));
    }
    let start = (top * row + left).min(luma.len());
    let size = (w as u32, h as u32);
    let half = (w / 2) as u32 * 2;
    Ok(Planes {
        size,
        luma: (&luma[start..], row as u32),
        chroma: (chroma, half),
        bt709: h >= 720,
        full: false,
    })
}

impl Clips {
    pub(crate) fn open(&mut self, path: &Path) -> Result<NativeWindow, Error> {
        if let Some(clip) = self.0.iter().find(|c| c.path == path) {
            return clip.reader.window().map_err(media);
        }
        let reader = ImageReader::new(EDGE, EDGE, ImageFormat::YUV_420_888, IMAGES).map_err(media)?;
        let window = reader.window().map_err(media)?;
        self.0.push(Clip {
            path: path.to_owned(),
            reader,
            yuv: None,
            chroma: Vec::new(),
            epoch: 0,
            warned: false,
        });
        Ok(window)
    }

    pub(crate) fn close(&mut self, path: &Path) {
        self.0.retain(|clip| clip.path != path);
    }

    pub(crate) fn forget(&mut self) {
        self.0.iter_mut().for_each(|clip| clip.yuv = None);
    }

    pub(crate) fn frames<'a>(
        &'a mut self,
        r: &Renderer,
        epoch: u64,
        wanted: &'a [(String, PathBuf)],
    ) -> Vec<(&'a str, &'a GpuImage)> {
        let mut shown = Vec::new();
        for clip in &mut self.0 {
            if clip.epoch != epoch {
                (clip.yuv, clip.epoch) = (None, epoch);
            }
            let converted = match clip.reader.acquire_latest_image() {
                Ok(AcquireResult::Image(image)) => planes(&image, &mut clip.chroma).and_then(|planes| {
                    crate::counted::gpu(|| r.convert_yuv(&mut clip.yuv, &planes).map(drop))
                        .map_err(Error::from)
                        .inspect(|()| {
                            SHOWN.fetch_add(1, Relaxed);
                        })
                }),
                Ok(_) => Ok(()),
                Err(e) => Err(media(e)),
            };
            if let Err(e) = converted {
                failed(&e, &mut clip.warned);
            }
            let clip: &'a Clip = clip;
            let Some(yuv) = clip.yuv.as_ref() else { continue };
            shown.extend(
                wanted.iter().filter(|(_, p)| *p == clip.path).map(|(id, _)| (id.as_str(), yuv.image())),
            );
        }
        shown
    }
}
