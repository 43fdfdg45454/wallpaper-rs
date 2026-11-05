use super::Reel;
use super::output::{Output, Sink};
use super::power::Power;
use super::slot::PlayerSlot;
use super::surface::Screen;
use std::collections::{BTreeMap, HashMap};
use wprs_core::HdrChoice;
use wprs_core::hdr::headroom;
use wprs_render::{Encoding, RenderError, Renderer};
use wprs_wayland::Mastered;

#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize)]
pub struct Hdr {
    pub active: bool,
    pub offered: bool,
    pub display: bool,
    pub peak_nits: Option<f32>,
    pub reference_nits: Option<f32>,
    pub reason: &'static str,
    pub encoding: &'static str,
    #[serde(skip)]
    pub described: Option<Mastered>,
}

impl Hdr {
    pub fn headroom(&self) -> f32 {
        if self.active { headroom(self.peak_nits, self.reference_nits) } else { 1.0 }
    }

    pub fn reference(&self) -> f32 {
        self.reference_nits.filter(|n| n.is_finite() && *n > 0.0).unwrap_or(wprs_render::REFERENCE_NITS)
    }

    pub fn mastered(&self) -> Mastered {
        let nits = |v: f32| v.round().clamp(1.0, 10_000.0) as u32;
        Mastered { max_cll: nits(self.headroom() * self.reference()), max_fall: nits(self.reference()) }
    }
}

impl Output {
    fn top(&self) -> bool {
        let bright = |s: &super::scene::SceneState| s.spec.pack.scene.canvas.bright();
        self.bright_video
            || self.shown.scene.as_ref().is_some_and(|s| s.quality().quality == "q3" && bright(s))
    }
}

impl Screen {
    fn light(
        &mut self,
        renderer: &Renderer,
        choice: HdrChoice,
        (battery, top): (bool, bool),
    ) -> Result<Hdr, RenderError> {
        let light = self.window.light();
        let (peak_nits, reference_nits) = (light.map(|l| l.peak_nits), light.map(|l| l.reference_nits));
        let display = light.is_some_and(|l| l.hdr);
        let Some(surface) = self.surface.as_mut() else {
            let reason = choice.decide((false, display), battery, top).1;
            return Ok(Hdr { display, peak_nits, reference_nits, reason, ..Hdr::default() });
        };
        renderer.own_pq(surface, self.window.describes_pq())?;
        let (want, reason) = choice.decide((surface.hdr_offered(), display), battery, top);
        renderer.light(surface, want)?;
        let (active, offered, encoding) = (surface.hdr(), surface.hdr_offered(), surface.encoding());
        let reason = if want && !active { "display" } else { reason };
        let mut hdr = Hdr { active, offered, display, peak_nits, reference_nits, reason, ..Hdr::default() };
        surface.set_light(hdr.reference(), hdr.headroom());
        (hdr.encoding, hdr.described) =
            (encoding.name(), (encoding == Encoding::OwnPq).then(|| hdr.mastered()));
        self.window.describe(hdr.described);
        Ok(hdr)
    }
}

pub(super) fn light(output: &mut Output, renderer: &Renderer, power: Power) -> Result<(), RenderError> {
    let (choice, top) = (output.shown.tone.hdr, output.top());
    let found = match &mut output.sink {
        Sink::Surface(screen) => screen.light(renderer, choice, (power.on_battery, top))?,
        Sink::Stream(_) => {
            Hdr { reason: choice.decide((false, false), power.on_battery, top).1, ..Hdr::default() }
        }
    };
    if found.active != output.hdr.active {
        (output.dirty, output.composed) = (true, None);
    }
    output.hdr = found;
    Ok(())
}

pub(super) fn video(outputs: &BTreeMap<String, Output>, reel: &Reel) -> f32 {
    let shows = |o: &&Output| o.layers().any(|s| s.content.videos().iter().any(|(r, _)| *r == reel));
    let least = outputs.values().filter(shows).map(|o| o.hdr.headroom()).fold(f32::INFINITY, f32::min);
    if least.is_finite() { least } else { 1.0 }
}

pub(super) fn bright_videos(players: &HashMap<Reel, PlayerSlot>, outputs: &mut BTreeMap<String, Output>) {
    let bright: Vec<&Reel> = players.iter().filter(|(_, s)| s.bright()).map(|(r, _)| r).collect();
    for o in outputs.values_mut() {
        o.bright_video = o.shown.content.videos().iter().any(|(r, _)| bright.contains(r));
    }
}

#[cfg(test)]
mod tests;
