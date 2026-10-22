use super::lock;
use super::output::{Output, Sink, smoothstep};
use super::power::Power;
use super::slot::PlayerSlot;
use super::state::State;
use crate::fatal;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::time::{Duration, Instant};
use wprs_pipewire::{Exporter, VideoFrame};
use wprs_render::{RenderError, Renderer, log_allowed};

const TRANSITION_FPS: u32 = 60;

type Rate = (u32, Option<(u32, u32)>);

type Shared<'a> = (&'a Renderer, &'a Exporter, &'a HashMap<PathBuf, PlayerSlot>, Power, Option<(u32, u32)>);

fn interval(output: &Output, max_fps: u32, limits: Option<(u32, u32)>) -> Duration {
    let max_fps = limits.filter(|_| output.shown.relief.is_some()).map_or(max_fps, |(_, fps)| fps);
    let fps = if output.fading() { TRANSITION_FPS.max(max_fps) } else { max_fps };
    let scene = output.shown.scene.as_ref().filter(|_| !output.fading()).map(|s| s.interval);
    (Duration::from_secs(1) / fps.max(1)).max(scene.unwrap_or_default())
}

fn deadline(output: &Output, open: bool, (max_fps, limits): Rate, now: Instant) -> Option<Instant> {
    (output.dirty && open).then(|| output.last_render.map_or(now, |t| t + interval(output, max_fps, limits)))
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
        let (now, limits) = (Instant::now(), self.relief_limits());
        let rate = (self.settings.max_fps, limits);
        let open = |name: &String, o: &Output| o.open(self.exporter.is_streaming(name));
        let renders = self.outputs.iter().filter_map(|(name, o)| deadline(o, open(name, o), rate, now));
        let scenes = self.outputs.iter().filter(|(name, o)| o.active && open(name, o));
        let scenes = scenes.filter_map(|(_, o)| o.shown.scene.as_ref()?.due);
        renders
            .chain(scenes)
            .chain(self.players.values().filter_map(|s| s.due))
            .chain(self.starvation())
            .chain(self.backdrops())
            .chain(limits.and_then(|(_, fps)| self.motion.due(fps)))
            .min()
    }

    pub(super) fn render_due(&mut self) {
        let (now, limits, transition) =
            (Instant::now(), self.relief_limits(), self.settings.transition.as_secs_f32());
        let shared = (&self.renderer, &*self.exporter, &self.players, self.power, limits);
        for (name, output) in &mut self.outputs {
            let streaming = shared.1.is_streaming(name);
            if output.active && output.shown.scene.as_ref().and_then(|s| s.due).is_some_and(|at| at <= now) {
                output.dirty = true;
            }
            if deadline(output, output.open(streaming), (self.settings.max_fps, limits), now)
                .is_none_or(|at| at > now)
            {
                continue;
            }
            if !output.watched(streaming) {
                output.fade = None;
            }
            match draw(name, output, shared, transition, now) {
                Ok(()) if output.last_render == Some(now) => {
                    lock(&self.status).get_mut(name).into_iter().for_each(|s| s.frames += 1)
                }
                Ok(()) => {}
                Err(e) => {
                    static FAILURES: AtomicU64 = AtomicU64::new(0);
                    if log_allowed(&FAILURES) {
                        tracing::error!(output = name, "render failed: {e:#}");
                    }
                    output.dirty = false;
                }
            }
        }
        self.paint_backdrops(now);
    }
}

fn draw(
    name: &str,
    output: &mut Output,
    shared: Shared<'_>,
    transition: f32,
    now: Instant,
) -> anyhow::Result<()> {
    let (renderer, exporter, players, power, limits) = shared;
    let moved = limits.map(|(steps, _)| (output.offset, steps));
    if let Some(scene) = output.shown.scene.as_mut() {
        scene.render(renderer, output.size, power, players, output.active, now)?;
    }
    let progress = output.progress(now, output.shown.transition.map_or(transition, |d| d.as_secs_f32()));
    let opacity = if output.fade.is_some() { smoothstep(progress) } else { 1.0 };
    let shown = output.shown.layer(players, opacity, moved);
    let from = output.fade.as_ref().map(|(from, _)| from.layer(players, 1.0, moved));
    let layers = [from.unwrap_or(shown), shown];
    let layers = &layers[usize::from(from.is_none())..];
    let written = match &mut output.sink {
        Sink::Stream(target) => {
            let target = match target {
                Some(target) => target,
                empty => empty.insert(renderer.create_target(output.size.0, output.size.1)?),
            };
            renderer.render_layers_with(target, layers, |w, h, s, d| {
                exporter.write_frame(name, VideoFrame { width: w, height: h, stride: s, data: d })
            })
        }
        Sink::Surface(screen) => match screen.present(renderer, layers, name) {
            Ok(false) => return Ok(()),
            presented => presented.map(|_| Ok(())),
        },
    };
    if let Err(RenderError::DeviceLost(reason)) = &written {
        fatal(&format!("the GPU stopped responding ({reason})"));
    }
    written??;
    output.missing_frame = output.shown.content.video().is_some() && shown.image.is_none();
    output.last_render = Some(now);
    output.fade = output.fade.take().filter(|_| progress < 1.0);
    output.dirty = output.fading();
    output.stale = output.missing_frame || output.fade.is_some();
    match (output.animated(), &mut output.sink) {
        (true, Sink::Surface(screen)) => screen.due = None,
        (true, Sink::Stream(_)) => {}
        (false, Sink::Stream(target)) => *target = None,
        (false, Sink::Surface(screen)) => {
            screen.backdrop(renderer, &[output.shown.layer(players, 1.0, moved)], now)?
        }
    }
    Ok(())
}
