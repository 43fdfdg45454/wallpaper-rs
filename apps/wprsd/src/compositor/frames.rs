use super::output::{Output, smoothstep};
use super::state::State;
use super::{Content, OutputStatus, lock};
use std::sync::Arc;
use std::time::{Duration, Instant};
use wprs_pipewire::VideoFrame;

const TRANSITION_FPS: u32 = 60;

impl State {
    pub(super) fn start_fades(&mut self) {
        let now = Instant::now();
        for output in self.outputs.values_mut() {
            let slot = output.shown.content.video().and_then(|p| self.players.get(p));
            let ready = slot.is_none_or(|s| s.texture.is_some() || s.player.is_none());
            if let Some((_, start @ None)) = &mut output.fade
                && ready
            {
                *start = Some(now);
                output.dirty = true;
            }
        }
    }

    fn interval(&self, output: &Output) -> Duration {
        let fps =
            if output.fading() { TRANSITION_FPS.max(self.settings.max_fps) } else { self.settings.max_fps };
        Duration::from_secs(1) / fps.max(1)
    }

    pub(super) fn next_deadline(&self) -> Option<Instant> {
        let due = |o: &Output| o.last_render.map_or_else(Instant::now, |t| t + self.interval(o));
        self.outputs.values().filter(|o| o.dirty).map(due).min()
    }

    pub(super) fn render_due(&mut self) {
        let now = Instant::now();
        let due: Vec<String> = (self.outputs.iter())
            .filter(|(_, o)| o.dirty && o.last_render.is_none_or(|t| t + self.interval(o) <= now))
            .map(|(name, _)| name.clone())
            .collect();
        for name in due {
            if let Err(e) = self.render(&name, now) {
                tracing::error!(output = name, "render failed: {e:#}");
                self.outputs.get_mut(&name).into_iter().for_each(|o| o.dirty = false);
            }
        }
    }

    fn render(&mut self, name: &str, now: Instant) -> anyhow::Result<()> {
        let Some(output) = self.outputs.get_mut(name) else { return Ok(()) };
        let progress = output.progress(now, self.settings.transition.as_secs_f32());
        if output.target.is_none() {
            output.target = Some(self.renderer.create_target(output.size.0, output.size.1)?);
        }
        let mut layers: Vec<_> = output.fade.iter().map(|(from, _)| from.layer(&self.players, 1.0)).collect();
        let opacity = if output.fade.is_some() { smoothstep(progress) } else { 1.0 };
        layers.push(output.shown.layer(&self.players, opacity));
        let frame = self.renderer.render_layers(output.target.as_ref().expect("created above"), &layers)?;
        drop(layers);
        output.last_render = Some(now);
        if progress >= 1.0 {
            output.fade = None;
        }
        output.dirty = output.fading();
        let video =
            VideoFrame { width: frame.width, height: frame.height, stride: frame.stride, data: frame.data };
        Ok(self.exporter.push_frame(name, Arc::new(video))?)
    }

    pub(super) fn publish_status(&self) {
        let status = self.outputs.iter().map(|(name, o)| {
            let slot = o.shown.content.video().and_then(|p| self.players.get(p));
            let error = slot.and_then(|s| lock(&s.error).clone());
            let kind = match o.shown.content {
                Content::Color => "color",
                Content::Image(_) => "image",
                Content::Video(_) => "video",
            };
            let playing = slot.is_some_and(|s| s.playing && s.player.is_some()) && error.is_none();
            (name.clone(), OutputStatus { kind, playing, error })
        });
        *lock(&self.status) = status.collect();
    }
}
