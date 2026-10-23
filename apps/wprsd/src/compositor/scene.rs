use super::power::Power;
use super::slot::PlayerSlot;
use super::{SceneInput, SceneSpec};
use chrono::Timelike;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::time::{Duration, Instant};
use wprs_render::{GpuImage, RenderError, Renderer, SceneView, log_allowed};
use wprs_scene::{PointerState, Quality, Runtime, ScriptOutputs, Uses, Voice, Wake};
use wprs_script::{Error, Frame, Host};

const SCRIPT_HOST: &str = "/proc/self/exe";
const CALM: u32 = 30;
const POLL: f64 = 0.5;

pub(super) struct SceneState {
    pub(super) spec: SceneSpec,
    runtime: Runtime,
    still: bool,
    reads: (bool, bool),
    quality: Quality,
    uses: Uses,
    view: Option<SceneView>,
    tick: Option<Instant>,
    pointer: PointerState,
    host: Option<Host>,
    calm: u32,
    pub(super) error: Option<String>,
    pub(super) due: Option<Instant>,
    pub(super) moving: bool,
    pub(super) voices: Vec<Voice>,
    pub(super) heard: Option<Instant>,
}

fn after(seconds: f64) -> Duration {
    Duration::try_from_secs_f64(seconds.clamp(0.0, 86_400.0)).unwrap_or_default()
}

fn wall() -> f64 {
    let now = chrono::Utc::now().with_timezone(&crate::zone::Zone);
    f64::from(now.num_seconds_from_midnight()) + f64::from(now.nanosecond() % 1_000_000_000) / 1e9
}

impl SceneState {
    pub(super) fn new(spec: SceneSpec, power: Power) -> SceneState {
        let mut runtime = Runtime::new(spec.pack.scene.clone(), spec.values.clone());
        let still = power.still(&runtime);
        if still {
            tracing::debug!("the scene is too heavy for this device and stays at its poster time");
            runtime.seek(f64::from(runtime.scene.clock.poster_time));
        }
        let (scripts, seed) = (&spec.pack.scripts, runtime.scene.clock.seed);
        let host = (!still && runtime.uses().scripts)
            .then(|| Host::new(SCRIPT_HOST, &["--script-host"], scripts, seed));
        let read = |word| scripts.iter().any(|(_, source)| source.contains(word));
        let (reads, quality) = ((read("audio"), read("cursor")), power.quality(&runtime));
        let (view, tick, pointer, calm, error, due, uses) = Default::default();
        let (moving, voices, heard) = (!still, Vec::new(), None);
        let mut scene = SceneState {
            spec,
            runtime,
            still,
            reads,
            quality,
            uses,
            view,
            tick,
            pointer,
            host,
            calm,
            error,
            due,
            moving,
            voices,
            heard,
        };
        scene.uses = scene.needs(quality);
        scene
    }

    fn needs(&self, quality: Quality) -> Uses {
        if self.still {
            return Uses::default();
        }
        let uses = self.runtime.uses_at(quality);
        Uses { audio: uses.audio || self.reads.0, cursor: uses.cursor || self.reads.1, ..uses }
    }

    pub(super) fn pause(&mut self) {
        self.tick = None;
        if let Some(view) = self.view.as_mut() {
            view.trim();
            self.runtime.restart_particles();
        }
    }

    pub(super) fn uses(&self) -> Uses {
        self.uses
    }

    pub(super) fn playing(&self) -> bool {
        self.moving || self.host.is_some()
    }

    pub(super) fn update(&mut self, spec: SceneSpec) -> bool {
        let changed = self.runtime.set_props(spec.values.clone());
        (self.spec, self.uses) = (spec, self.needs(self.quality));
        changed
    }

    pub(super) fn input(&mut self, input: SceneInput) -> bool {
        match input {
            SceneInput::Audio(_, audio) => {
                let last = self.runtime.audio();
                let heard = self.uses.audio && *audio != *last && !(audio.silent && last.silent);
                self.runtime.set_audio(*audio);
                heard
            }
            SceneInput::Pointer(_, pointer) => {
                let moved = std::mem::replace(&mut self.pointer, pointer) != pointer;
                moved && self.uses.cursor
            }
            SceneInput::Script(_, outputs) => {
                let changed = self.runtime.set_script(outputs);
                self.calm = if changed { 0 } else { self.calm };
                changed
            }
        }
    }

    pub(super) fn script(&mut self) -> Option<ScriptOutputs> {
        let rt = &self.runtime;
        let (time, dt, props, audio, pointer) = (rt.time(), rt.dt(), rt.props(), rt.audio(), rt.pointer());
        match self.host.as_mut()?.update(&Frame { time, dt, props, audio, pointer }) {
            Ok(outputs) => outputs,
            Err(Error::Disabled) => {
                tracing::warn!(scene = self.spec.pack.id, "the scene scripts failed too often and are off");
                (self.host, self.error) = (None, Some(Error::Disabled.to_string()));
                None
            }
            Err(e) => {
                static FAILURES: AtomicU64 = AtomicU64::new(0);
                if log_allowed(&FAILURES) {
                    tracing::warn!(scene = self.spec.pack.id, "scene script: {e}");
                }
                None
            }
        }
    }

    pub(super) fn image(&self) -> Option<&GpuImage> {
        self.view.as_ref().and_then(SceneView::image)
    }

    pub(super) fn fps(&self, power: Power, refresh: u32) -> u32 {
        power.fps(&self.runtime, self.quality, refresh)
    }

    pub(super) fn render(
        &mut self,
        renderer: &Renderer,
        (size, refresh): ((u32, u32), u32),
        power: Power,
        players: &HashMap<PathBuf, PlayerSlot>,
        active: bool,
        now: Instant,
    ) -> Result<bool, RenderError> {
        self.due = None;
        let dt = self.step(active, now);
        let p = self.pointer;
        let pointer = self.runtime.pointer().follow(p.pos, p.buttons, dt as f32);
        self.runtime.set_pointer(pointer);
        self.runtime.set_wall(wall());
        let (quality, uses) = (power.quality(&self.runtime), self.uses);
        if quality != self.quality {
            (self.quality, self.uses) = (quality, self.needs(quality));
        }
        let scale = quality.render_scale();
        let inner = (
            (size.0 as f32 * scale).round().max(1.0) as u32,
            (size.1 as f32 * scale).round().max(1.0) as u32,
        );
        if self.view.is_none() {
            let view = renderer.scene_view(&self.spec.pack.scene, &self.assets())?;
            if view.gpu_particles() == Some(false) {
                tracing::debug!("particles run on the CPU with a smaller budget");
            }
            self.view = Some(view);
        }
        let view = self.view.as_mut().expect("created above");
        let frame = self.runtime.frame(view.view(inner), quality);
        let videos: Vec<(&str, &GpuImage)> = (self.spec.pack.videos.iter())
            .filter_map(|(id, path)| Some((id.as_str(), players.get(path)?.texture.as_ref()?)))
            .collect();
        renderer.render_scene(view, &self.runtime, &frame, inner, quality, &videos)?;
        let interval = Duration::from_secs(1) / power.fps(&self.runtime, quality, refresh);
        let wake = match self.host {
            _ if self.still => Wake::Idle,
            Some(_) if self.calm < CALM => Wake::Frame,
            Some(_) => frame.wake.min(Wake::At(POLL)),
            None => frame.wake,
        };
        self.calm = self.calm.saturating_add(1);
        self.moving = wake == Wake::Frame;
        self.due = match wake {
            Wake::Frame => Some(now + interval),
            Wake::At(seconds) => Some(now + after(seconds).max(interval)),
            Wake::Idle => None,
        };
        if self.due.is_none() {
            view.trim();
        }
        self.listen(interval, now);
        Ok(uses != self.uses)
    }

    fn step(&mut self, active: bool, now: Instant) -> f64 {
        let dt = self.tick.filter(|_| active).map_or(0.0, |t| now.saturating_duration_since(t).as_secs_f64());
        if active && !self.still {
            self.runtime.advance(dt);
        }
        self.tick = active.then_some(now);
        dt
    }

    pub(super) fn hold(&mut self) {
        self.tick = None;
    }

    pub(super) fn hear(&mut self, interval: Duration, now: Instant) {
        self.step(true, now);
        self.listen(interval, now);
    }

    fn listen(&mut self, interval: Duration, now: Instant) {
        if self.spec.pack.sounds.is_empty() {
            return;
        }
        let (voices, wake) = self.runtime.voices();
        self.voices = voices;
        self.heard = match wake {
            Wake::Frame => Some(now + interval),
            Wake::At(seconds) => Some(now + after(seconds).max(interval)),
            Wake::Idle => None,
        };
    }

    fn assets(&self) -> wprs_render::SceneAssets {
        let mut assets = self.spec.pack.assets.clone();
        assets.images.extend(self.spec.extra.iter().cloned());
        assets
    }
}
