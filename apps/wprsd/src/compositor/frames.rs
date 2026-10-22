use super::fatal;
use super::output::{Output, smoothstep};
use super::slot::PlayerSlot;
use super::state::State;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::time::{Duration, Instant};
use wprs_pipewire::VideoFrame;
use wprs_render::{RenderError, Renderer, log_allowed};

const TRANSITION_FPS: u32 = 60;
const POOL: usize = 3;

fn interval(output: &Output, max_fps: u32) -> Duration {
    let fps = if output.fading() { TRANSITION_FPS.max(max_fps) } else { max_fps };
    Duration::from_secs(1) / fps.max(1)
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
        let due =
            |o: &Output| o.last_render.map_or_else(Instant::now, |t| t + interval(o, self.settings.max_fps));
        let renders = self.outputs.values().filter(|o| o.dirty).map(due);
        renders.chain(self.players.values().filter_map(|s| s.due)).min()
    }

    pub(super) fn render_due(&mut self) {
        let (now, max_fps, transition) =
            (Instant::now(), self.settings.max_fps, self.settings.transition.as_secs_f32());
        let due = |o: &Output| o.dirty && o.last_render.is_none_or(|t| t + interval(o, max_fps) <= now);
        for (name, output) in self.outputs.iter_mut().filter(|(_, o)| due(o)) {
            let frame = draw(output, &self.renderer, &self.players, transition, now);
            if let Err(RenderError::DeviceLost(reason)) = &frame {
                fatal(&format!("the GPU stopped responding ({reason})"));
            }
            let pushed =
                frame.map_err(anyhow::Error::from).and_then(|f| Ok(self.exporter.push_frame(name, f)?));
            if let Err(e) = pushed {
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
    output: &mut Output,
    renderer: &Renderer,
    players: &HashMap<PathBuf, PlayerSlot>,
    transition: f32,
    now: Instant,
) -> Result<Arc<VideoFrame>, RenderError> {
    let progress = output.progress(now, transition);
    let target = match &mut output.target {
        Some(target) => target,
        empty => empty.insert(renderer.create_target(output.size.0, output.size.1)?),
    };
    let shown = output.shown.layer(players, if output.fade.is_some() { smoothstep(progress) } else { 1.0 });
    let from = output.fade.as_ref().map(|(from, _)| from.layer(players, 1.0));
    let layers = [from.unwrap_or(shown), shown];
    let frame = free_frame(&mut output.frames);
    let pixels = Arc::get_mut(frame).expect("free frames are not shared");
    renderer.render_layers(target, &layers[usize::from(from.is_none())..], &mut pixels.data)?;
    ((pixels.width, pixels.height), pixels.stride) = (target.size(), target.stride());
    output.missing_frame = output.shown.content.video().is_some() && shown.image.is_none();
    let frame = frame.clone();
    output.last_render = Some(now);
    output.fade = output.fade.take().filter(|_| progress < 1.0);
    output.dirty = output.fading();
    if !output.animated() {
        (output.target, output.frames) = (None, Vec::new());
    }
    Ok(frame)
}

fn free_frame(pool: &mut Vec<Arc<VideoFrame>>) -> &mut Arc<VideoFrame> {
    let index = pool.iter_mut().position(|f| Arc::get_mut(f).is_some()).unwrap_or_else(|| {
        if pool.len() >= POOL {
            pool.remove(0);
        }
        pool.push(Arc::new(VideoFrame { width: 0, height: 0, stride: 0, data: Vec::new() }));
        pool.len() - 1
    });
    &mut pool[index]
}
