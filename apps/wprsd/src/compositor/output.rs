use super::slot::PlayerSlot;
use super::{Content, Spec};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use wprs_core::Fit;
use wprs_pipewire::VideoFrame;
use wprs_render::{GpuImage, Layer, Target};

pub(super) struct Shown {
    pub(super) content: Content,
    pub(super) fit: Fit,
    pub(super) background: [u8; 3],
    pub(super) image: Option<GpuImage>,
}

impl Shown {
    pub(super) fn matches(&self, spec: &Spec) -> bool {
        self.content.same_source(&spec.content) && self.fit == spec.fit && self.background == spec.background
    }

    pub(super) fn layer<'a>(&'a self, players: &'a HashMap<PathBuf, PlayerSlot>, opacity: f32) -> Layer<'a> {
        let image = match &self.content {
            Content::Color => None,
            Content::Image(_) => self.image.as_ref(),
            Content::Video(path) => players.get(path).and_then(|p| p.texture.as_ref()),
        };
        Layer { image, fit: self.fit, background: self.background, opacity }
    }
}

pub(super) struct Output {
    pub(super) size: (u32, u32),
    pub(super) shown: Shown,
    pub(super) fade: Option<(Shown, Option<Instant>)>,
    pub(super) active: bool,
    pub(super) dirty: bool,
    pub(super) missing_frame: bool,
    pub(super) target: Option<Target>,
    pub(super) frames: Vec<Arc<VideoFrame>>,
    pub(super) last_render: Option<Instant>,
}

impl Output {
    pub(super) fn new(size: (u32, u32), shown: Shown) -> Self {
        let (fade, target, frames, last_render) = (None, None, Vec::new(), None);
        Self {
            size,
            shown,
            fade,
            active: true,
            dirty: true,
            missing_frame: false,
            target,
            frames,
            last_render,
        }
    }

    pub(super) fn show(&mut self, shown: Shown, size: (u32, u32), transition: bool) {
        let resized = std::mem::replace(&mut self.size, size) != size;
        let animate = transition && !resized && self.active && self.last_render.is_some();
        let old = std::mem::replace(&mut self.shown, shown);
        self.fade = (animate && !old.content.same_source(&self.shown.content)).then_some((old, None));
        self.target = self.target.take().filter(|_| !resized);
        self.dirty = true;
    }

    pub(super) fn layers(&self) -> impl Iterator<Item = &Shown> {
        std::iter::once(&self.shown).chain(self.fade.as_ref().map(|(from, _)| from))
    }

    pub(super) fn uses_video(&self, path: &PathBuf) -> bool {
        self.layers().any(|s| s.content.video() == Some(path))
    }

    pub(super) fn animated(&self) -> bool {
        self.fade.is_some() || self.layers().any(|s| s.content.video().is_some())
    }

    pub(super) fn fading(&self) -> bool {
        matches!(self.fade, Some((_, Some(_))))
    }

    pub(super) fn progress(&self, now: Instant, duration: f32) -> f32 {
        match &self.fade {
            Some((_, Some(start))) => {
                (now.saturating_duration_since(*start).as_secs_f32() / duration.max(1e-3)).min(1.0)
            }
            Some((_, None)) => 0.0,
            None => 1.0,
        }
    }
}

pub(super) fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
