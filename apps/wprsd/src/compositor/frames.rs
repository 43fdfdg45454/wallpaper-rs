use super::output::{Output, smoothstep};
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

type Shared<'a> = (&'a Renderer, &'a Exporter, &'a HashMap<PathBuf, PlayerSlot>);

fn interval(output: &Output, max_fps: u32) -> Duration {
    let fps = if output.fading() { TRANSITION_FPS.max(max_fps) } else { max_fps };
    Duration::from_secs(1) / fps.max(1)
}

fn deadline(output: &Output, streaming: bool, max_fps: u32, now: Instant) -> Option<Instant> {
    let wanted = output.dirty && (streaming || output.stale);
    wanted.then(|| output.last_render.map_or(now, |t| t + interval(output, max_fps)))
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
        let (now, max_fps) = (Instant::now(), self.settings.max_fps);
        let streaming = |name: &String| self.exporter.is_streaming(name);
        let renders = self.outputs.iter().filter_map(|(name, o)| deadline(o, streaming(name), max_fps, now));
        renders.chain(self.players.values().filter_map(|s| s.due)).min()
    }

    pub(super) fn render_due(&mut self) {
        let (now, max_fps, transition) =
            (Instant::now(), self.settings.max_fps, self.settings.transition.as_secs_f32());
        let shared = (&self.renderer, &*self.exporter, &self.players);
        for (name, output) in &mut self.outputs {
            let streaming = shared.1.is_streaming(name);
            if deadline(output, streaming, max_fps, now).is_none_or(|at| at > now) {
                continue;
            }
            if !streaming {
                output.fade = None;
            }
            if let Err(e) = draw(name, output, shared, transition, now) {
                static FAILURES: AtomicU64 = AtomicU64::new(0);
                if log_allowed(&FAILURES) {
                    tracing::error!(output = name, "render failed: {e:#}");
                }
                output.dirty = false;
            }
        }
    }
}

fn draw(
    name: &str,
    output: &mut Output,
    shared: Shared<'_>,
    transition: f32,
    now: Instant,
) -> anyhow::Result<()> {
    let (renderer, exporter, players) = shared;
    let progress = output.progress(now, transition);
    let target = match &mut output.target {
        Some(target) => target,
        empty => empty.insert(renderer.create_target(output.size.0, output.size.1)?),
    };
    let shown = output.shown.layer(players, if output.fade.is_some() { smoothstep(progress) } else { 1.0 });
    let from = output.fade.as_ref().map(|(from, _)| from.layer(players, 1.0));
    let layers = [from.unwrap_or(shown), shown];
    let written =
        renderer.render_layers_with(target, &layers[usize::from(from.is_none())..], |w, h, s, d| {
            exporter.write_frame(name, VideoFrame { width: w, height: h, stride: s, data: d })
        });
    if let Err(RenderError::DeviceLost(reason)) = &written {
        fatal(&format!("the GPU stopped responding ({reason})"));
    }
    written??;
    output.missing_frame = output.shown.content.video().is_some() && shown.image.is_none();
    output.last_render = Some(now);
    output.fade = output.fade.take().filter(|_| progress < 1.0);
    output.dirty = output.fading();
    output.stale = output.missing_frame || output.fade.is_some();
    if !output.animated() {
        output.target = None;
    }
    Ok(())
}
