use super::output::Output;
use super::power::Power;
use super::state::State;
use std::time::Instant;

const SIDE: u32 = 320;
const LIGHTS_FPS: u32 = 30;

impl Output {
    pub(super) fn would_light(&self, name: &str) -> bool {
        let scene = self.shown.scene.as_ref().is_some_and(|s| s.sends_rgb());
        let page = self.shown.content.page().is_some_and(|p| p.pack.uses[3]);
        !self.active && self.covered && self.awake && (scene || page) && crate::inputs::lights(name)
    }

    pub(super) fn lights_only(&self, name: &str) -> bool {
        self.would_light(name) && crate::rgb::follows()
    }

    pub(super) fn page_view(&self, name: &str, streaming: bool, power: Power) -> ((bool, bool), u32) {
        match self.lights_only(name) {
            true => ((true, true), self.fps(power).min(LIGHTS_FPS)),
            false => ((self.active, self.watched(streaming)), self.fps(power)),
        }
    }

    pub(super) fn lit_only(&mut self, now: Instant) -> bool {
        let next = now + crate::rgb::EVERY;
        self.shown.scene.iter_mut().for_each(|s| s.due = s.due.map(|d| d.max(next)));
        (self.last_render, self.failed, self.dirty, self.stale) = (Some(now), 0, false, true);
        self.composed = None;
        false
    }
}

impl State {
    pub(super) fn want_lights(&self) {
        self.outputs.iter().for_each(|(name, o)| crate::rgb::want(name, o.would_light(name)));
    }
}

pub(super) fn small((w, h): (u32, u32)) -> (u32, u32) {
    let most = w.max(h).max(1);
    let side = |v: u32| (u64::from(v) * u64::from(SIDE) / u64::from(most)).max(1) as u32;
    if most <= SIDE { (w, h) } else { (side(w), side(h)) }
}

#[cfg(test)]
mod tests {
    use super::super::Content;
    use super::super::output::{Shown, Sink};
    use super::*;
    use wprs_core::Fit;

    #[test]
    fn covered_lights_draw_small_and_never_present() {
        assert_eq!(
            [small((3840, 2160)), small((200, 100)), small((1080, 2400))],
            [(320, 180), (200, 100), (144, 320)]
        );
        let (content, fit, background) = (Content::Color, Fit::Stretch, [0; 3]);
        let (image, transition, uv, relief, scene) = (None, None, None, None, None);
        let shown = Shown {
            content,
            fit,
            background,
            image,
            transition,
            uv,
            relief,
            scene,
            tone: Default::default(),
        };
        let mut output = Output::new((16, 9), shown, Sink::Stream(None), 60);
        (output.active, output.covered) = (false, true);
        assert!(!output.lights_only("TEST"), "an output without lights followed them");
        output.awake = false;
        assert!(!output.would_light("TEST"), "a paused, locked or battery output asked for the lights");
        let now = Instant::now();
        assert!(!output.lit_only(now), "a frame for the lights counted as drawn");
        assert!(
            !output.dirty && output.stale && output.last_render == Some(now) && output.composed.is_none()
        );
    }
}
