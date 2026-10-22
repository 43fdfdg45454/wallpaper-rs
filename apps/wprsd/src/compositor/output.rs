use super::scene::SceneState;
use super::slot::PlayerSlot;
use super::surface::Screen;
use super::{Content, Spec};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use wprs_core::{Fit, UvRect};
use wprs_render::{GpuImage, Layer, Target};

pub(super) struct Shown {
    pub(super) content: Content,
    pub(super) fit: Fit,
    pub(super) background: [u8; 3],
    pub(super) image: Option<GpuImage>,
    pub(super) transition: Option<Duration>,
    pub(super) uv: Option<UvRect>,
    pub(super) scene: Option<SceneState>,
}

impl Shown {
    pub(super) fn matches(&self, spec: &Spec) -> bool {
        self.content.same_look(&spec.content) && self.fit == spec.fit && self.background == spec.background
    }

    pub(super) fn layer<'a>(&'a self, players: &'a HashMap<PathBuf, PlayerSlot>, opacity: f32) -> Layer<'a> {
        let (image, fit) = match &self.content {
            Content::Color => (None, self.fit),
            Content::Image(_) => (self.image.as_ref(), self.fit),
            Content::Video(path) => (players.get(path).and_then(|p| p.texture.as_ref()), self.fit),
            Content::Scene(_) => (self.scene.as_ref().and_then(SceneState::image), Fit::Stretch),
        };
        Layer { image, fit, background: self.background, opacity, uv: self.uv }
    }
}

pub(super) enum Sink {
    Stream(Option<Target>),
    Surface(Screen),
}

pub(super) struct Output {
    pub(super) size: (u32, u32),
    pub(super) shown: Shown,
    pub(super) fade: Option<(Shown, Option<Instant>)>,
    pub(super) active: bool,
    pub(super) dirty: bool,
    pub(super) missing_frame: bool,
    pub(super) stale: bool,
    pub(super) sink: Sink,
    pub(super) last_render: Option<Instant>,
}

impl Output {
    pub(super) fn new(size: (u32, u32), shown: Shown, sink: Sink) -> Self {
        let (fade, last_render) = (None, None);
        let (active, dirty, missing_frame, stale) = (true, true, false, true);
        Self { size, shown, fade, active, dirty, missing_frame, stale, sink, last_render }
    }

    pub(super) fn show(&mut self, shown: Shown, size: (u32, u32), transition: bool) {
        let resized = std::mem::replace(&mut self.size, size) != size;
        let animate = transition && !resized && self.active && self.last_render.is_some();
        let old = std::mem::replace(&mut self.shown, shown);
        self.fade = (animate && !old.content.same_source(&self.shown.content)).then_some((old, None));
        if let Sink::Stream(target) = &mut self.sink
            && resized
        {
            *target = None;
        }
        (self.dirty, self.stale) = (true, true);
    }

    pub(super) fn open(&self, streaming: bool) -> bool {
        match &self.sink {
            Sink::Stream(_) => streaming || self.stale,
            Sink::Surface(screen) => screen.ready(),
        }
    }

    pub(super) fn watched(&self, streaming: bool) -> bool {
        streaming || matches!(self.sink, Sink::Surface(_))
    }

    pub(super) fn layers(&self) -> impl Iterator<Item = &Shown> {
        std::iter::once(&self.shown).chain(self.fade.as_ref().map(|(from, _)| from))
    }

    pub(super) fn uses_video(&self, path: &PathBuf) -> bool {
        self.layers().any(|s| s.content.videos().contains(&path))
    }

    pub(super) fn animated(&self) -> bool {
        let moving =
            |s: &Shown| !s.content.videos().is_empty() || s.scene.as_ref().is_some_and(|sc| sc.due.is_some());
        self.fade.is_some() || self.layers().any(moving)
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
