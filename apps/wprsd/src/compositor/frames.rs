use super::lock;
use super::output::{Output, Sink, smoothstep};
use super::power::Power;
use super::slot::PlayerSlot;
use super::state::State;
use crate::fatal;
use anyhow::Context;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::time::{Duration, Instant};
use wprs_pipewire::{Exporter, Grant, VideoFrame};
use wprs_render::{Exported, RenderError, Renderer, log_allowed};

const TRANSITION_FPS: u32 = 60;

type Shared<'a> = (&'a Renderer, &'a Exporter, &'a HashMap<PathBuf, PlayerSlot>, Power, Option<u32>);

fn interval(output: &Output, power: Power) -> Duration {
    let rate = power.rate(output.refresh, None);
    let fading = if power.on_battery { rate } else { TRANSITION_FPS.max(rate) };
    let fading = if output.refresh > 0 { fading.min(output.refresh) } else { fading };
    Duration::from_secs(1) / if output.fading() { fading } else { output.fps(power) }
}

fn deadline(output: &Output, open: bool, power: Power, now: Instant) -> Option<Instant> {
    (output.dirty && open).then(|| output.last_render.map_or(now, |t| t + interval(output, power)))
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
            .chain(self.starvation())
            .chain(self.backdrops())
            .chain(moving.and_then(|fps| self.motion.due(fps)))
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
            output.dirty |= shared.1.starved(name);
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
) -> anyhow::Result<bool> {
    let (renderer, exporter, players, power, limits) = shared;
    let moved = limits.map(|steps| (output.offset, steps));
    let refresh = output.refresh;
    if let Some(scene) = output.shown.scene.as_mut()
        && scene.render(renderer, (output.size, refresh), power, players, output.active, now)?
    {
        super::feed::declare(name, Some(scene));
    }
    let progress = output.progress(now, output.shown.transition.map_or(transition, |d| d.as_secs_f32()));
    let opacity = if output.fade.is_some() { smoothstep(progress) } else { 1.0 };
    let shown = output.shown.layer(players, opacity, moved);
    let from = output.fade.as_ref().map(|(from, _)| from.layer(players, 1.0, moved));
    let layers = [from.unwrap_or(shown), shown];
    let layers = &layers[usize::from(from.is_none())..];
    let written = match &mut output.sink {
        Sink::Stream(target) => match exporter.lease(name) {
            Grant::Buffer(lease) => {
                *target = None;
                let image = lease.image.downcast_ref::<Exported>().context("foreign stream buffer")?;
                renderer.render_exported(image, layers).map(|()| exporter.present(lease))
            }
            Grant::Wait => {
                output.last_render = Some(now);
                return Ok(false);
            }
            Grant::Copy => {
                let target = match target {
                    Some(target) => target,
                    empty => empty.insert(renderer.create_target(output.size.0, output.size.1)?),
                };
                renderer.render_layers_with(target, layers, |w, h, s, d| {
                    exporter.write_frame(name, VideoFrame { width: w, height: h, stride: s, data: d })
                })
            }
        },
        Sink::Surface(screen) => match screen.present(renderer, layers, name) {
            Ok(false) => return Ok(false),
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
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::super::output::Shown;
    use super::super::scene::SceneState;
    use super::super::{Content, SceneSpec};
    use super::*;
    use crate::scenes::ScenePack;
    use std::sync::Arc;
    use wprs_core::Fit;
    use wprs_scene::{Scene, Tier};

    fn snowfall() -> SceneSpec {
        let scene =
            Arc::new(Scene::from_json(include_str!("../../../../scenes/snowfall/scene.json")).unwrap());
        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../../../../scenes/snowfall/manifest.json")).unwrap();
        let properties = serde_json::from_value(manifest["properties"].clone()).unwrap();
        let (path, id, title, presets, assets) = Default::default();
        let (videos, sounds, clips, scripts) = Default::default();
        let pack = Arc::new(ScenePack {
            path,
            id,
            title,
            languages: Vec::new(),
            locale: String::new(),
            scene,
            properties,
            presets,
            assets,
            videos,
            sounds,
            clips,
            scripts,
        });
        SceneSpec { values: pack.values(&[]), pack, extra: Vec::new(), hold: None }
    }

    #[test]
    fn fades_never_outrun_a_known_display_or_the_battery_cap() {
        let color = || {
            let (content, fit, background) = (Content::Color, Fit::Stretch, [0; 3]);
            let (image, transition, uv, relief, scene) = (None, None, None, None, None);
            Shown { content, fit, background, image, transition, uv, relief, scene }
        };
        let fade = |refresh, on_battery| {
            let mut output = Output::new((16, 9), color(), Sink::Stream(None), refresh);
            output.fade = Some((color(), Some(Instant::now())));
            interval(&output, Power { tier: Tier::T3, on_battery, max_fps: 0 })
        };
        let every = |fps: u32| Duration::from_secs(1) / fps;
        let fades = [fade(20, false), fade(0, false), fade(144, false), fade(144, true)];
        assert_eq!(fades, [every(20), every(60), every(144), every(30)]);
    }

    #[test]
    fn a_scene_that_follows_the_display_asks_for_120_fps_on_a_120_hz_stream() {
        let renderer = match Renderer::new() {
            Ok(renderer) => renderer,
            Err(e) => return assert!(std::env::var_os("WPRS_REQUIRE_GPU").is_none(), "GPU required: {e}"),
        };
        let cadence = |power: Power| {
            let (spec, now) = (snowfall(), Instant::now());
            let scene = Some(SceneState::new(spec.clone(), power));
            let (content, image, transition, uv, relief) = (Content::Scene(spec), None, None, None, None);
            let shown = Shown {
                content,
                fit: Fit::Stretch,
                background: [0; 3],
                image,
                transition,
                uv,
                relief,
                scene,
            };
            let mut output = Output::new((16, 9), shown, Sink::Stream(None), 120);
            let scene = output.shown.scene.as_mut().expect("a scene");
            scene
                .render(&renderer, (output.size, output.refresh), power, &HashMap::new(), true, now)
                .unwrap();
            let due = scene.due.map(|at| at - now);
            output.last_render = Some(now);
            (due, deadline(&output, true, power, now).map(|at| at - now))
        };
        let every = |fps: u32| Some(Duration::from_secs(1) / fps);
        let mains = Power { tier: Tier::T3, on_battery: false, max_fps: 0 };
        assert_eq!(cadence(mains), (every(120), every(120)));
        assert_eq!(cadence(Power { on_battery: true, ..mains }), (every(30), every(30)));
        assert_eq!(cadence(Power { max_fps: 60, ..mains }), (every(60), every(60)));
    }
}
