use crate::clips::Clips;
use crate::framing::{self, Framed};
use crate::gpu::{self, Gpu, Key};
use crate::power::{Power, delay_ms};
use crate::relief::{self, Look as Seen, Raised, Tilted, layer};
use crate::settings::{self, Look};
use crate::stage::Stage;
use crate::{Error, choice, store};
use ndk::native_window::{FrameRateCompatibility, NativeWindow};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use wprs_core::Fit;
use wprs_render::{GpuImage, RenderError, Surface};

mod parked;
mod scenes;

pub(crate) use parked::forget;

pub(crate) static FRAMES: AtomicU64 = AtomicU64::new(0);

const RETRY_MS: i64 = 50;
const SLACK_NS: i64 = 2_000_000;
const ATTEMPTS: u32 = 3;

type Fading = (Arc<GpuImage>, (Fit, Option<[f32; 4]>), Option<i64>);

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
    size: (u32, u32),
    offset: f32,
    shown: f32,
    dirty: bool,
    presented: i64,
    rate: u32,
    failures: u32,
    video: bool,
    window: NativeWindow,
}

fn size(width: i32, height: i32) -> (u32, u32) {
    (width.max(1).unsigned_abs(), height.max(1).unsigned_abs())
}

impl Engine {
    pub(crate) fn open(window: NativeWindow, preview: bool) -> Self {
        let size = size(window.width(), window.height());
        let kept = parked::take(preview);
        let (stage, image, raised, epoch) =
            kept.map_or((None, None, None, 0), |k| (k.stage, k.image, k.raised, k.epoch));
        let chosen = choice::shown(preview).map(|shown| shown.uid);
        let image = image.filter(|(key, _)| chosen.as_ref() == Some(&key.item));
        let (target, fading, checked, rate, failures) = (None, None, (epoch, 0, (0, 0)), 0, 0);
        let (offset, shown, dirty, presented, video, look) = (0.5, 0.5, true, 0, false, settings::look());
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
            size,
            offset,
            shown,
            dirty,
            presented,
            rate,
            failures,
            video,
            window,
        }
    }

    pub(crate) fn video(&mut self) -> Option<String> {
        self.forget_old_gpu();
        let uri = choice::shown(self.preview).filter(|shown| shown.video);
        let uri = uri.map(|shown| format!("file://{}", shown.path.display()));
        if uri.is_some() {
            self.hint(0);
            (self.target, self.image, self.fading, self.raised, self.stage) = (None, None, None, None, None);
            gpu::trim();
        }
        if self.video != uri.is_some() {
            (self.video, self.checked, self.dirty) = (uri.is_some(), (0, 0, (0, 0)), true);
        }
        uri
    }

    pub(crate) fn resize(&mut self, width: i32, height: i32) {
        let size = size(width, height);
        self.dirty |= size != self.size;
        self.size = size;
    }

    pub(crate) fn offset(&mut self, x: f32) -> bool {
        self.offset = x.clamp(0.0, 1.0);
        let pans = self.look.fit == Fit::Fill && self.offset != self.shown;
        !self.video && self.stage.is_none() && (pans || self.raised.is_some()) && Power::now().moving()
    }

    pub(crate) fn parallax(&mut self) -> u32 {
        if self.video { 0 } else { self.tilted.rate(self.preview) }
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

    fn frame(&mut self, nanos: i64, fps: u32, power: Power) -> Result<(u32, i64), RenderError> {
        if self.target.as_ref().is_none_or(|(gpu, _)| !gpu.current()) {
            (self.target, self.fading, self.dirty) = (None, None, true);
            let opened = gpu::open(&self.window, self.size)?;
            if std::mem::replace(&mut self.checked.0, 0) != opened.0.epoch {
                (self.image, self.raised, self.checked) = (None, None, (0, 0, (0, 0)));
            } else {
                self.checked = (opened.0.epoch, 0, (0, 0));
            }
            self.target = Some(opened);
        }
        let epoch = self.target.as_ref().map_or(0, |(gpu, _)| gpu.epoch);
        let check = (epoch, store::generation(), self.size);
        if self.checked != check {
            let old = std::mem::replace(&mut self.checked, check).0;
            let (size, shown) = (self.size, choice::shown(self.preview));
            if shown.as_ref().is_some_and(|shown| shown.video) {
                return Ok((0, 0));
            }
            self.stage(shown.as_ref());
            if old != epoch {
                self.stage.iter_mut().for_each(Stage::drop_view);
            }
            let Some((gpu, _)) = self.target.as_ref() else { return Ok((0, 0)) };
            let before = (self.look.fit, self.crop);
            self.look = settings::look();
            let shown = shown.filter(|shown| !shown.scene);
            let framed =
                shown.as_ref().map(|s| framing::framed(&s.uid, &s.path, self.look.fit, self.preview));
            self.look.fit = framed.as_ref().map_or(self.look.fit, |framed| framed.fit);
            let geometry = framed.as_ref().and_then(|f| f.develop.as_ref()).map(|edit| &edit.geometry);
            let raise = shown.as_ref().map(|s| (&*s.uid, &*s.path));
            self.dirty |= relief::raise(&mut self.raised, gpu, raise, geometry);
            let fit = self.look.fit;
            let wanted = shown.zip(framed).map(|(shown, framed)| {
                let look = format!(
                    "{:?}{:?}",
                    framed.develop,
                    framed.crop.map(|[.., w, h]| w.min(h).log2().floor())
                );
                (Key { epoch: gpu.epoch, item: shown.uid, fit, size, look }, shown.path, framed)
            });
            self.framed = wanted.as_ref().map(|(key, _, framed)| (key.item.clone(), framed.clone()));
            if self.image.as_ref().map(|(key, _)| key) != wanted.as_ref().map(|(key, ..)| key) {
                let loaded = wanted
                    .and_then(|(key, path, framed)| Some((key.clone(), gpu.texture(key, &path, &framed)?)));
                let previous = std::mem::replace(&mut self.image, loaded);
                let other = |old: &Key| self.image.as_ref().is_some_and(|(new, _)| new.item != old.item);
                let fade =
                    previous.filter(|(old, _)| (old.epoch, old.size) == (gpu.epoch, size) && other(old));
                let fades = fps > 0 && self.image.is_some() && !self.look.fade.is_zero();
                self.fading = fade.filter(|_| fades).map(|(_, old)| (old, before, None));
                self.dirty = true;
                gpu::trim();
            }
        }
        if let Some(next) = self.shoot(nanos, fps, power)? {
            return Ok(next);
        }
        let Some((gpu, surface)) = self.target.as_mut() else { return Ok((0, 0)) };
        if power.moving() && self.offset != self.shown {
            (self.shown, self.dirty) = (self.offset, true);
        }
        let crop = self.framed.as_ref().and_then(|(uid, framed)| framed.shown(uid, self.preview));
        if crop != self.crop {
            (self.crop, self.dirty) = (crop, true);
        }
        let raised =
            self.raised.as_mut().map(|r| r.step(self.offset, (nanos, fps), power.moving(), self.size));
        let (moved, tilting) = raised.unwrap_or_default();
        self.dirty |= moved;
        let duration = self.look.fade.as_nanos().max(1) as f32;
        let start = self.fading.as_mut().map(|(.., start)| *start.get_or_insert(nanos));
        let progress = start.map_or(1.0, |start| ((nanos - start) as f32 / duration).clamp(0.0, 1.0));
        if self.fading.is_some() && (fps == 0 || progress >= 1.0) {
            (self.fading, self.dirty) = (None, true);
            gpu::trim();
        }
        let animating = |on: bool| if on { (fps, delay_ms(fps)) } else { (0, 0) };
        if !self.dirty && self.fading.is_none() {
            return Ok(animating(tilting));
        }
        let (fit, background, pan, size) = (self.look.fit, self.look.background, self.shown, self.size);
        let look = Seen { fit, background, pan, crop: self.crop };
        let current = self.image.as_ref().map(|(_, image)| image.as_ref());
        let raised = self.raised.as_ref().map(|raised| (raised, power.charging));
        let layers = match &self.fading {
            Some((old, (fit, crop), _)) => {
                let was = Seen { fit: *fit, crop: *crop, ..look };
                [layer(Some(old), 1.0, &was, size, None), layer(current, progress, &look, size, raised)]
            }
            None => [layer(current, 1.0, &look, size, raised), layer(None, 0.0, &look, size, None)],
        };
        self.dirty = !gpu.renderer.present_layers(surface, size, &layers, || {})?;
        if !self.dirty {
            self.presented = nanos;
            FRAMES.fetch_add(1, Relaxed);
        }
        Ok(animating(self.fading.is_some() || tilting))
    }
}
