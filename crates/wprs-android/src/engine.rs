use crate::clips::Clips;
use crate::framing::Framed;
use crate::gpu::{self, Gpu, Key};
use crate::power::Power;
use crate::relief::{Raised, Tilted};
use crate::settings::{self, Look};
use crate::stage::Stage;
use crate::{Error, blend, choice};
use ndk::native_window::{FrameRateCompatibility, NativeWindow};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use wprs_core::Fit;
use wprs_render::{GpuImage, Surface};

mod fade;

use fade::Fading;
mod parked;
mod picture;
mod scenes;

pub(crate) use parked::forget;

pub(crate) static FRAMES: AtomicU64 = AtomicU64::new(0);

const RETRY_MS: i64 = 50;
const SLACK_NS: i64 = 2_000_000;
const ATTEMPTS: u32 = 3;

pub(crate) struct Engine {
    target: Option<(Gpu, Surface)>,
    image: Option<(Key, Arc<GpuImage>)>,
    fading: Option<Fading>,
    raised: Option<Raised>,
    framed: Option<(String, Framed)>,
    crop: Option<[f32; 4]>,
    stage: Option<Stage>,
    clips: Clips,
    tilted: Tilted,
    checked: (u64, u64, (u32, u32)),
    look: Look,
    preview: bool,
    lock: bool,
    size: (u32, u32),
    offset: f32,
    shown: f32,
    dirty: bool,
    presented: i64,
    rate: u32,
    failures: u32,
    video: bool,
    film: Option<PathBuf>,
    direct: Option<PathBuf>,
    handover: bool,
    hidden: bool,
    window: NativeWindow,
}

fn size(width: i32, height: i32) -> (u32, u32) {
    (width.max(1).unsigned_abs(), height.max(1).unsigned_abs())
}

impl Engine {
    pub(crate) fn open(window: NativeWindow, preview: bool, lock: bool) -> Self {
        let size = size(window.width(), window.height());
        let kept = parked::take(preview, lock);
        let (stage, image, raised, epoch) =
            kept.map_or((None, None, None, 0), |k| (k.stage, k.image, k.raised, k.epoch));
        let chosen = choice::shown_on(preview, lock).map(|shown| shown.uid);
        let image = image.filter(|(key, _)| chosen.as_ref() == Some(&key.item));
        let stage = stage.filter(|stage| blend::resumes(Some(&stage.uid), chosen.as_deref()));
        let (target, fading, checked, rate, failures) = (None, None, (epoch, 0, (0, 0)), 0, 0);
        let (offset, shown, dirty, presented, video, look) = (0.5, 0.5, true, 0, false, settings::look(lock));
        Self {
            target,
            image,
            fading,
            raised,
            framed: None,
            crop: None,
            stage,
            clips: Clips::default(),
            tilted: Tilted::default(),
            checked,
            look,
            preview,
            lock,
            size,
            offset,
            shown,
            dirty,
            presented,
            rate,
            failures,
            video,
            film: None,
            direct: None,
            handover: false,
            hidden: true,
            window,
        }
    }

    pub(crate) fn screen(&mut self, lock: bool) {
        if self.lock != lock {
            (self.lock, self.checked, self.dirty, self.tilted) =
                (lock, (0, 0, (0, 0)), true, Tilted::default());
        }
    }

    pub(crate) fn resize(&mut self, width: i32, height: i32) {
        let size = size(width, height);
        self.dirty |= size != self.size;
        self.size = size;
    }

    pub(crate) fn offset(&mut self, x: f32) -> bool {
        self.offset = x.clamp(0.0, 1.0);
        let pans = self.look.fit == Fit::Fill && self.offset != self.shown;
        let tilts = self.stage.as_mut().is_some_and(|stage| stage.offset(self.offset));
        let pictured = self.stage.is_none() && (pans || self.raised.is_some());
        !self.video && (tilts || pictured) && Power::now().moving()
    }

    pub(crate) fn parallax(&mut self) -> u32 {
        if self.video { 0 } else { self.tilted.rate(self.preview, self.lock) }
    }

    pub(crate) fn draw(&mut self, nanos: i64, force: bool) -> Result<i64, Error> {
        if self.video {
            return Ok(0);
        }
        let power = Power::now();
        let fps = power.fps(self.preview);
        let due = self.presented + 1_000_000_000 / i64::from(fps.max(1)) - SLACK_NS;
        if !force && nanos < due {
            return Ok(((due - nanos) / 1_000_000).max(1));
        }
        self.dirty |= force;
        let next = self.frame(nanos, fps, power).inspect_err(|e| {
            (self.target, self.image, self.fading, self.raised, self.dirty) = (None, None, None, None, true);
            log::warn!("{e}");
        });
        self.failures = if next.as_ref().is_ok_and(|_| !self.dirty) { 0 } else { self.failures + 1 };
        if std::mem::take(&mut self.handover) {
            return Ok(-1);
        }
        let (hint, delay) = match next {
            Err(e) if self.failures >= ATTEMPTS => return Err(e.into()),
            result => result.unwrap_or((0, 0)),
        };
        self.hint(hint);
        Ok(match () {
            _ if delay > 0 => delay,
            _ if self.dirty && self.failures < ATTEMPTS => RETRY_MS,
            _ => 0,
        })
    }

    fn forget_old_gpu(&mut self) {
        if self.target.as_ref().is_some_and(|(gpu, _)| !gpu.current()) {
            (self.target, self.image, self.fading, self.raised, self.dirty) = (None, None, None, None, true);
            self.checked = (0, 0, (0, 0));
            self.stage.iter_mut().for_each(Stage::drop_view);
            self.clips.forget();
            gpu::trim();
        }
    }

    fn hint(&mut self, fps: u32) {
        if self.rate != fps {
            self.rate = fps;
            let _ = self.window.set_frame_rate(fps as f32, FrameRateCompatibility::Default);
        }
    }
}
