use super::parallax::SOFTWARE;
use super::power::Power;
use super::scene::SceneState;
use super::{Content, Reel};
use std::time::Instant;
use wprs_core::edit::Parallax;
use wprs_core::settings::fps::lower;
use wprs_core::{Ages, Region};
use wprs_render::GpuImage;
use wprs_scene::Tier;

mod shown;

pub(super) use shown::{Shown, Sink};

pub(super) type Raised = (GpuImage, Parallax);

type Composed = (u64, (u32, u32), Option<([u32; 2], u32)>);

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
    pub(super) offset: [f32; 2],
    pub(super) moving: bool,
    pub(super) refresh: u32,
    pub(super) covered: bool,
    pub(super) composed: Option<Composed>,
    pub(super) failed: u32,
    pub(super) ages: Ages,
}

impl Output {
    pub(super) fn new(size: (u32, u32), shown: Shown, sink: Sink, refresh: u32) -> Self {
        let (fade, last_render) = (None, None);
        let (active, dirty, missing_frame, stale, moving) = (true, true, false, true, false);
        let (offset, covered, failed) = ([0.0; 2], false, 0);
        Self {
            size,
            shown,
            fade,
            active,
            dirty,
            missing_frame,
            stale,
            sink,
            last_render,
            offset,
            moving,
            refresh,
            covered,
            failed,
            composed: None,
            ages: Ages::default(),
        }
    }

    pub(super) fn unwatched(&mut self, streaming: bool) {
        if let Sink::Stream(target) = &mut self.sink
            && !streaming
            && !self.stale
        {
            *target = None;
        }
    }

    pub(super) fn guard(&mut self, on: bool) {
        self.dirty |= self.shown.scene.as_mut().is_some_and(|s| s.guard(on));
    }

    pub(super) fn fps(&self, power: Power) -> u32 {
        let rate = power.rate(self.refresh, None);
        match (&self.shown.scene, &self.shown.relief, &self.shown.content) {
            (Some(scene), ..) => scene.fps(power, self.refresh),
            (None, Some(_), _) if power.tier == Tier::T0 => rate.min(SOFTWARE.1),
            (.., Content::Video(m)) => lower(power.video(self.refresh), m.max_fps),
            (.., Content::Web(p)) => lower(rate, p.pack.max_fps),
            _ => rate,
        }
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
        (self.dirty, self.stale, self.failed, self.composed) = (true, true, 0, None);
    }

    pub(super) fn unchanged(&mut self, moved: Option<([f32; 2], u32)>, starved: bool) -> bool {
        let scene = self.shown.scene.as_ref().filter(|_| self.fade.is_none());
        let shift = moved.filter(|_| self.shown.relief.is_some()).map(|(o, s)| (o.map(f32::to_bits), s));
        let key = scene.and_then(SceneState::image).map(|i| (i.id(), self.size, shift));
        let clean = !self.stale && scene.is_some_and(SceneState::clean);
        let settled = match &self.sink {
            Sink::Surface(screen) => screen.settled(),
            Sink::Stream(_) => !starved,
        };
        let same = key.is_some() && std::mem::replace(&mut self.composed, key) == key;
        same && clean && settled
    }

    pub(super) fn damage(&self) -> Option<Region> {
        let plain = self.fade.is_none() && self.shown.relief.is_none() && self.shown.uv.is_none();
        self.shown.scene.as_ref().filter(|_| plain && !self.stale)?.damage
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

    pub(super) fn uses_video(&self, reel: &Reel) -> bool {
        self.loudest(reel).is_some()
    }

    pub(super) fn loudest(&self, reel: &Reel) -> Option<u32> {
        let volumes = self.layers().flat_map(|s| s.content.videos()).filter(|(r, _)| *r == reel);
        volumes.map(|(_, volume)| volume).max()
    }

    pub(super) fn animated(&self) -> bool {
        let moving =
            |s: &Shown| !s.content.videos().is_empty() || s.scene.as_ref().is_some_and(|sc| sc.due.is_some());
        let moving = |s: &Shown| moving(s) || s.content.page().is_some();
        self.fade.is_some() || self.moving || self.layers().any(moving)
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
