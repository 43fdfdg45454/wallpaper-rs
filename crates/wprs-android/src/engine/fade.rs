use super::{Engine, FRAMES};
use crate::gpu;
use crate::power::{Power, delay_ms};
use crate::relief::{Look as Seen, layer};
use crate::settings::{self, Look};
use crate::stage::Stage;
use crate::{blend, choice};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering::Relaxed;
use wprs_core::Fit;
use wprs_render::{GpuImage, RenderError};

const FILM: &str = "\u{0}film";
const GONE: &str = "\u{0}gone";

pub(crate) enum Gone {
    Picture(Arc<GpuImage>, (Fit, Option<[f32; 4]>)),
    Scene(Box<Stage>),
    Film(PathBuf, (Fit, [u8; 3])),
}

pub(crate) struct Fading {
    gone: Gone,
    start: Option<i64>,
    asked: Option<i64>,
}

impl Fading {
    pub(super) fn new(gone: Gone) -> Self {
        Fading { gone, start: None, asked: None }
    }
}

pub(super) fn direct(look: &Look) -> bool {
    matches!(look.fit, Fit::Fill | Fit::Stretch) && look.flip == [false; 2] && look.grade.is_none()
}

impl Engine {
    pub(crate) fn video(&mut self) -> Option<String> {
        self.forget_old_gpu();
        let shown = choice::shown_on(self.preview, self.lock).filter(|shown| shown.video);
        let flip = shown.as_ref().map_or([false; 2], |s| s.flip);
        let reel = crate::reel::shown(shown.as_ref());
        self.dirty |= reel != self.reel;
        self.reel = reel;
        let shown = shown.map(|shown| shown.path);
        let current = self.direct.clone().or_else(|| self.film.clone());
        if shown != current {
            let look = settings::Look { flip, ..settings::look(self.lock) };
            let fps = Power::now().fps(self.preview);
            let film = (look.fit, look.background);
            let gone = match (self.direct.take().or(self.film.take()), shown.is_some()) {
                (Some(path), _) => Some(Gone::Film(path, film)),
                (None, true) => match (self.image.take(), self.stage.take()) {
                    (Some((_, image)), _) => Some(Gone::Picture(image, (self.look.fit, self.crop))),
                    (_, Some(stage)) => stage.image().is_some().then(|| Gone::Scene(Box::new(stage))),
                    _ => None,
                },
                (None, false) => None,
            };
            let fades =
                |gone: &Gone| blend::fades(look.fade, fps, self.hidden, matches!(gone, Gone::Film(..)));
            self.fading = match gone {
                Some(gone) if fades(&gone) => Some(Fading::new(gone)),
                Some(_) => None,
                None => self.fading.take(),
            };
            (self.film, self.look, self.dirty, self.still) = (shown, look, true, None);
        }
        let plain = direct(&settings::Look { flip, ..settings::look(self.lock) }) && self.reel.direct();
        if self.film.is_some() && self.fading.is_none() && plain {
            self.direct = self.film.take();
            (self.target, self.image, self.raised, self.stage) = (None, None, None, None);
            gpu::trim();
        } else if self.direct.is_some() && !plain {
            (self.film, self.dirty) = (self.direct.take(), true);
        }
        if self.video != self.direct.is_some() {
            (self.video, self.checked, self.dirty) = (self.direct.is_some(), (0, 0, (0, 0)), true);
        }
        if self.direct.is_some() {
            self.hint(self.reel.cap());
        }
        self.unreport();
        self.direct.as_ref().map(|path| format!("file://{}", path.display()))
    }

    pub(crate) fn reel(&self) -> Option<String> {
        self.direct.as_ref().or(self.film.as_ref()).map(|_| self.reel.told().to_string())
    }

    pub(crate) fn film(&self) -> Option<String> {
        self.film.as_ref().and_then(|path| path.to_str()).map(String::from)
    }

    pub(super) fn reels(&self) -> Vec<(String, PathBuf)> {
        let own = self.stage.as_ref().map(|stage| stage.videos().to_vec()).unwrap_or_default();
        let gone = match self.fading.as_ref().map(|f| &f.gone) {
            Some(Gone::Film(path, _)) => Some((GONE.to_owned(), path.clone())),
            _ => None,
        };
        own.into_iter().chain(gone).chain(self.film.clone().map(|path| (FILM.to_owned(), path))).collect()
    }

    pub(super) fn compose(&mut self, nanos: i64, fps: u32, power: Power) -> Result<(u32, i64), RenderError> {
        let reels = self.reels();
        let Some((gpu, surface)) = self.target.as_mut() else { return Ok((0, 0)) };
        let (r, film) = (&gpu.renderer, self.film.as_deref());
        let headroom = match (film, &self.stage) {
            (Some(film), _) => crate::hdr::light(r, surface, power.charging, crate::media::bright(film))?,
            (None, None) => crate::hdr::dark(r, surface).map(|()| 1.0)?,
            (None, Some(_)) => 1.0,
        };
        self.clips.take(r, (gpu.epoch, headroom));
        let new = reels.iter().any(|(_, path)| self.clips.fresh(path));
        let since = match self.still {
            Some(at) if !film.is_some_and(|film| self.clips.fresh(film)) => at,
            _ => nanos,
        };
        self.still = Some(since);
        let frames = self.clips.shown(&reels);
        let reel = |id: &str| frames.iter().find(|(key, _)| *key == id).map(|(_, image)| *image);
        let size = self.size;
        let (fit, background, flip, grade) =
            (self.look.fit, self.look.background, self.look.flip, self.look.grade);
        let look = Seen { fit, background, pan: self.shown, crop: self.crop, flip, grade };
        let stretched = Seen { fit: Fit::Stretch, crop: None, ..look };
        if let Some(stage) = self.stage.as_mut() {
            let r = &gpu.renderer;
            let tier = crate::power::tier(r.software(), r.compute()).min(r.tier());
            stage.reports = (!self.preview).then_some(usize::from(self.lock));
            stage.render(r, (size, nanos, fps), (power, tier), &frames, true)?;
        }
        let (now, now_look) = match (&self.film, &self.stage, &self.image) {
            (Some(_), ..) => (reel(FILM), look),
            (_, Some(stage), _) => (stage.image(), stretched),
            (.., Some((_, image))) => (Some(image.as_ref()), look),
            _ => (None, look),
        };
        let had = match self.fading.as_ref().map(|f| &f.gone) {
            Some(Gone::Picture(..)) => true,
            Some(Gone::Scene(stage)) => stage.image().is_some(),
            Some(Gone::Film(..)) => reel(GONE).is_some(),
            None => false,
        };
        let duration = self.look.fade.as_nanos().max(1) as f32;
        let progress = self.fading.as_mut().map_or(1.0, |fading| {
            let asked = *fading.asked.get_or_insert(nanos);
            let ready = blend::begins(had, now.is_some(), nanos - asked);
            let start = fading.start.or(ready.then_some(nanos));
            fading.start = start;
            start.map_or(0.0, |start| ((nanos - start) as f32 / duration).clamp(0.0, 1.0))
        });
        let (was, was_look) = match self.fading.as_ref().map(|f| &f.gone) {
            Some(Gone::Picture(image, (fit, crop))) => {
                (Some(image.as_ref()), Seen { fit: *fit, crop: *crop, ..look })
            }
            Some(Gone::Scene(stage)) => (stage.image(), stretched),
            Some(Gone::Film(_, (fit, background))) => {
                (reel(GONE), Seen { fit: *fit, background: *background, crop: None, ..look })
            }
            None => (None, look),
        };
        let waiting = self.fading.as_ref().is_some_and(|fading| fading.start.is_none());
        let mut layers = match self.fading {
            Some(_) => [layer(was, 1.0, &was_look, size, None), layer(now, progress, &now_look, size, None)],
            None => [layer(now, 1.0, &now_look, size, None), layer(None, 0.0, &now_look, size, None)],
        };
        let shown = &mut layers[usize::from(self.fading.is_some())];
        if self.film.is_some() && self.reel.framed() {
            shown.uv = shown.uv.map(|uv| uv.framed(self.reel.zoom, self.reel.pan));
        }
        if !waiting && (now.is_some() || was.is_some()) && (new || self.dirty || self.fading.is_some()) {
            self.dirty = !gpu.renderer.present_layers(surface, size, &layers, || {})?;
            if !self.dirty {
                self.presented = nanos;
                FRAMES.fetch_add(1, Relaxed);
            }
        }
        if self.fading.is_some() && (fps == 0 || progress >= 1.0) {
            self.fading = None;
            let flip = self.look.flip;
            self.handover = self.film.is_some()
                && self.reel.direct()
                && direct(&settings::Look { flip, ..settings::look(self.lock) });
            gpu::trim();
        }
        let ended = blend::rests(self.reel.once, now.is_some(), nanos - since);
        let busy = self.fading.is_some() || (self.film.is_some() && power.moving() && !ended);
        if !busy || fps == 0 {
            self.still = None;
        }
        Ok(if busy && fps > 0 { (fps, delay_ms(fps)) } else { (0, 0) })
    }
}
