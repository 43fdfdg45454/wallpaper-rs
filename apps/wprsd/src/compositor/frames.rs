use super::output::Output;
use super::power::Power;
use super::slot::PlayerSlot;
use super::state::State;
use super::{Reel, lock};
use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::time::{Duration, Instant};
use wprs_pipewire::Exporter;
use wprs_render::{Renderer, log_allowed};

mod draw;

use draw::draw;

const TRANSITION_FPS: u32 = 60;
const RETRY: Duration = Duration::from_secs(1);
const RETRY_MAX: Duration = Duration::from_secs(60);

type Shared<'a> = (&'a Renderer, &'a Exporter, &'a HashMap<Reel, PlayerSlot>, Power, Option<u32>);

fn interval(output: &Output, power: Power) -> Duration {
    let rate = power.rate(output.refresh, None);
    let fading = if power.on_battery { rate } else { TRANSITION_FPS.max(rate) };
    let fading = if output.refresh > 0 { fading.min(output.refresh) } else { fading };
    Duration::from_secs(1) / if output.fading() { fading } else { output.fps(power) }
}

fn backoff(failed: u32) -> Duration {
    match failed {
        0 => Duration::ZERO,
        n => (RETRY * 2u32.pow((n - 1).min(6))).min(RETRY_MAX),
    }
}

fn deadline(output: &Output, open: bool, power: Power, now: Instant) -> Option<Instant> {
    let wait = interval(output, power).max(backoff(output.failed));
    (output.dirty && open).then(|| output.last_render.map_or(now, |t| t + wait))
}

impl State {
    pub(super) fn start_fades(&mut self) {
        let now = Instant::now();
        for output in self.outputs.values_mut() {
            let slot = output.shown.content.video().and_then(|p| self.players.get(p));
            let ready = slot.is_none_or(|s| s.texture.is_some() || s.error.is_some());
            if let Some((_, start @ None)) = &mut output.fade
                && ready
            {
                *start = Some(now);
                output.dirty = true;
            }
        }
    }

    pub(super) fn next_deadline(&self) -> Option<Instant> {
        let now = Instant::now();
        let open = |name: &String, o: &Output| o.open(self.exporter.is_streaming(name));
        let renders = self.outputs.iter().filter_map(|(name, o)| deadline(o, open(name, o), self.power, now));
        let moving = self.outputs.values().filter(|o| o.moving).map(|o| o.fps(self.power)).max();
        let scenes = self.outputs.iter().filter(|(name, o)| o.active && open(name, o));
        let scenes = scenes.filter_map(|(_, o)| o.shown.scene.as_ref()?.due);
        renders
            .chain(scenes)
            .chain(self.players.values().filter_map(|s| s.due))
            .chain(self.heard())
            .chain(self.page_due())
            .chain(self.starvation())
            .chain(self.backdrops())
            .chain(moving.and_then(|fps| self.motion.due(fps)))
            .min()
    }

    pub(super) fn render_due(&mut self) {
        let (now, limits, transition) =
            (Instant::now(), self.relief_limits(), self.settings.transition.as_secs_f32());
        let shared = (self.renderer, &*self.exporter, &self.players, self.power, limits);
        for (name, output) in &mut self.outputs {
            let streaming = shared.1.is_streaming(name);
            if output.active && output.shown.scene.as_ref().and_then(|s| s.due).is_some_and(|at| at <= now) {
                output.dirty = true;
            }
            output.dirty |= shared.1.starved(name);
            output.unwatched(streaming);
            if deadline(output, output.open(streaming), self.power, now).is_none_or(|at| at > now) {
                continue;
            }
            if !output.watched(streaming) {
                output.fade = None;
            }
            match draw(name, output, shared, transition, now) {
                Ok(true) => lock(&self.status).get_mut(name).into_iter().for_each(|s| s.frames += 1),
                Ok(false) => {}
                Err(e) => {
                    static FAILURES: AtomicU64 = AtomicU64::new(0);
                    if log_allowed(&FAILURES) {
                        tracing::error!(output = name, "render failed: {e:#}");
                    }
                    (output.failed, output.last_render) = (output.failed.saturating_add(1), Some(now));
                    tracing::debug!(output = name, retry = ?backoff(output.failed), "drawing again later");
                }
            }
        }
        self.paint_backdrops(now);
    }
}

#[cfg(test)]
mod tests;
