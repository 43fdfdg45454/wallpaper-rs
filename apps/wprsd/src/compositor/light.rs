use super::output::{Output, Sink};
use super::power::Power;
use super::surface::Screen;
use wprs_core::HdrChoice;
use wprs_core::hdr::headroom;
use wprs_render::{RenderError, Renderer};

#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize)]
pub struct Hdr {
    pub active: bool,
    pub offered: bool,
    pub display: bool,
    pub peak_nits: Option<f32>,
    pub reference_nits: Option<f32>,
    pub reason: &'static str,
}

impl Hdr {
    pub fn headroom(&self) -> f32 {
        if self.active { headroom(self.peak_nits, self.reference_nits) } else { 1.0 }
    }
}

impl Output {
    fn top(&self) -> bool {
        let bright = |s: &super::scene::SceneState| s.spec.pack.scene.canvas.bright();
        self.shown.scene.as_ref().is_some_and(|s| s.quality().quality == "q3" && bright(s))
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
        let (want, reason) = choice.decide((surface.hdr_offered(), display), battery, top);
        renderer.light(surface, want)?;
        let (active, offered) = (surface.hdr(), surface.hdr_offered());
        let reason = if want && !active { "display" } else { reason };
        Ok(Hdr { active, offered, display, peak_nits, reference_nits, reason })
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

#[cfg(test)]
mod tests;
