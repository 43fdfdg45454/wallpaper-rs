use super::power::Power;
use super::slot::PlayerSlot;
use super::{Reel, SceneInput, SceneSpec};
use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::time::{Duration, Instant};
use wprs_render::{GpuImage, RenderError, Renderer, SceneView, log_allowed};
use wprs_scene::{
    Course, Fired, MAX_FIRED, PointerState, Quality, Runtime, ScriptApi, ScriptOutputs, Uses, Values, Voice,
    Wake,
};
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
    exposed: Values,
    pressed: Vec<String>,
    fired: Vec<Fired>,
    courses: Vec<Course>,
    zone: (i64, String),
    restock: bool,
}

fn after(seconds: f64) -> Duration {
    Duration::try_from_secs_f64(seconds.clamp(0.0, 86_400.0)).unwrap_or_default()
}

impl SceneState {
    pub(super) fn new(spec: SceneSpec, power: Power) -> SceneState {
        let mut runtime = Runtime::new(spec.pack.scene.clone(), spec.values.clone());
        spec.pack.inputs.into_iter().for_each(|allowed| runtime.declare(allowed));
        runtime.set_language(&spec.pack.locale);
        spec.pack.assets.sheets.iter().for_each(|(id, d)| runtime.set_sheet(id, d.clone()));
        runtime.set_lengths(spec.pack.lengths.clone());
        runtime.set_playing(spec.clips.iter().map(|(asset, _)| asset.clone()).collect());
        let still = spec.hold.is_some() || power.still(&runtime);
        match spec.hold {
            Some(time) => runtime.seek(time),
            None if still => {
                tracing::debug!("the scene is too heavy for this device and stays at its poster time");
                runtime.seek(f64::from(runtime.scene.clock.poster_time));
            }
            None => {}
        }
        let (scripts, seed) = (&spec.pack.scripts, runtime.seed);
        let named = (spec.pack.locale.as_str(), &spec.pack.labels);
        let host = (!still && runtime.uses().scripts)
            .then(|| Host::new(SCRIPT_HOST, &["--script-host"], scripts, seed, named));
        let exposed = wprs_scene::exposed(&spec.pack.props.properties, &spec.values);
        let declares =
            |api| spec.pack.scene.scripts.iter().any(|s: &wprs_scene::Script| s.uses.contains(&api));
        let read = |word, api| declares(api) || scripts.iter().any(|(_, source)| source.contains(word));
        let reads = (read("audio", ScriptApi::Audio), read("cursor", ScriptApi::Pointer));
        let reads = spec.pack.inputs.map_or(reads, |u| (reads.0 && u.audio, reads.1 && u.cursor));
        let (reads, quality) = (reads, power.quality(&runtime));
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
            exposed,
            pressed: Vec::new(),
            fired: Vec::new(),
            courses: Vec::new(),
            zone: (i64::MIN, String::new()),
            restock: false,
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

    pub(super) fn time(&self) -> f64 {
        self.runtime.time()
    }

    pub(super) fn resume(&mut self, time: f64) {
        if !self.still {
            self.runtime.seek(time);
        }
    }

    pub(super) fn uses(&self) -> Uses {
        self.uses
    }

    pub(super) fn playing(&self) -> bool {
        self.moving || self.host.is_some()
    }

    pub(super) fn calm(&mut self, calm: bool) -> bool {
        self.runtime.set_reduced_motion(calm)
    }

    pub(super) fn press(&mut self, id: &str) {
        self.runtime.press(id);
        self.pressed.push(id.to_owned());
        self.calm = 0;
    }

    pub(super) fn update(&mut self, spec: SceneSpec) -> bool {
        let mut changed = self.runtime.set_props(spec.values.clone());
        let media = self.spec.media() != spec.media();
        if media {
            self.runtime.set_playing(spec.clips.iter().map(|(asset, _)| asset.clone()).collect());
        }
        self.restock |= changed || media;
        self.exposed = wprs_scene::exposed(&spec.pack.props.properties, &spec.values);
        if spec.hold != self.spec.hold {
            spec.hold.into_iter().for_each(|time| self.runtime.seek(time));
            (self.still, self.moving, changed) = (spec.hold.is_some(), spec.hold.is_none(), true);
        }
        (self.spec, self.uses) = (spec, self.needs(self.quality));
        changed || media
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
        let fresh = self.runtime.take_fired();
        let room = MAX_FIRED.saturating_sub(self.fired.len());
        self.fired.extend(fresh.into_iter().take(room));
        let rt = &self.runtime;
        rt.fill_playbacks(&mut self.courses);
        let minute = rt.wall().div_euclid(60.0) as i64;
        if self.zone.0 != minute {
            self.zone = (minute, crate::zone::posix());
        }
        let (time, dt, props, audio, pointer) = (rt.time(), rt.dt(), &self.exposed, rt.audio(), rt.pointer());
        let actions = &std::mem::take(&mut self.pressed);
        let (wall, zone, timelines, fired) =
            (rt.wall(), self.zone.1.as_str(), self.courses.as_slice(), &self.fired);
        let frame = Frame { time, dt, props, audio, pointer, actions, wall, zone, timelines, fired };
        let sent = self.host.as_mut()?.update(&frame);
        if !matches!(sent, Ok(None)) {
            self.fired.clear();
        }
        match sent {
            Ok(outputs) => outputs,
            Err(Error::Disabled) => {
                tracing::warn!(
                    scene = self.spec.pack.props.id,
                    "the scene scripts failed too often and are off"
                );
                (self.host, self.error) = (None, Some(Error::Disabled.to_string()));
                None
            }
            Err(e) => {
                static FAILURES: AtomicU64 = AtomicU64::new(0);
                if log_allowed(&FAILURES) {
                    tracing::warn!(scene = self.spec.pack.props.id, "scene script: {e}");
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
        players: &HashMap<Reel, PlayerSlot>,
        active: bool,
        now: Instant,
    ) -> Result<bool, RenderError> {
        self.due = None;
        let dt = self.step(active, now);
        let p = self.pointer;
        let pointer = self.runtime.pointer().follow(p.pos, p.buttons, dt as f32);
        self.runtime.set_pointer(pointer);
        self.runtime.set_wall(crate::zone::wall());
        let (quality, uses) = (power.quality(&self.runtime), self.uses);
        if quality != self.quality {
            (self.quality, self.uses) = (quality, self.needs(quality));
        }
        let scale = quality.render_scale();
        let inner = (
            (size.0 as f32 * scale).round().max(1.0) as u32,
            (size.1 as f32 * scale).round().max(1.0) as u32,
        );
        if let Some(view) = self.view.as_mut().filter(|_| std::mem::take(&mut self.restock)) {
            let assets = assets(&self.spec);
            renderer.refresh_scene(view, (&self.spec.pack.scene, self.runtime.props()), &assets)?;
        }
        if self.view.is_none() {
            let view =
                renderer.scene_view(&self.spec.pack.scene, &assets(&self.spec), self.runtime.props())?;
            if view.gpu_particles() == Some(false) {
                tracing::debug!("particles run on the CPU with a smaller budget");
            }
            self.view = Some(view);
        }
        let view = self.view.as_mut().expect("created above");
        let frame = self.runtime.frame(view.view(inner), quality);
        let videos: Vec<(&str, &GpuImage)> = self
            .spec
            .reels()
            .filter_map(|(id, reel, _)| Some((id, players.get(reel)?.texture.as_ref()?)))
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

    pub(super) fn tracks(&self) -> Vec<wprs_video::Track> {
        let sounds = self.spec.pack.scene.sounds();
        let fades = |s: &wprs_scene::Sound| (s.fade_in, s.fade_out);
        let lists = self.runtime.playlists().iter().cloned().zip(sounds.map(fades));
        lists.map(|(list, (fade_in, fade_out))| wprs_video::Track { list, fade_in, fade_out }).collect()
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
}

fn assets(spec: &SceneSpec) -> wprs_render::SceneAssets {
    let mut assets = spec.pack.assets.clone();
    assets.images.extend(spec.extra.iter().cloned());
    assets
}

#[cfg(test)]
mod tests {
    use super::super::Content;
    use super::*;
    use crate::scenes::ScenePack;
    use std::sync::Arc;
    use wprs_scene::{Scene, Tier};

    #[test]
    fn new_media_of_the_same_scene_keep_its_clock_and_view() {
        let renderer = match Renderer::new() {
            Ok(renderer) => renderer,
            Err(e) => return assert!(std::env::var_os("WPRS_REQUIRE_GPU").is_none(), "GPU required: {e}"),
        };
        let scene = Scene::from_json(include_str!("../../../../scenes/snowfall/scene.json")).unwrap();
        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../../../../scenes/snowfall/manifest.json")).unwrap();
        let properties = serde_json::from_value(manifest["properties"].clone()).unwrap();
        let pack = Arc::new(ScenePack::bare(scene, properties, Vec::new()));
        let spec = SceneSpec {
            values: pack.props.values(&[]),
            pack,
            extra: Vec::new(),
            clips: Vec::new(),
            hold: None,
        };
        let extra = vec![("photo".to_string(), Arc::new(image::RgbaImage::new(2, 2)))];
        let turned = SceneSpec { extra, ..spec.clone() };
        let (old, new) = (Content::Scene(spec.clone()), Content::Scene(turned.clone()));
        assert!(old.same_source(&new) && !old.same_look(&new));
        let power = Power { tier: Tier::T3, on_battery: false, max_fps: 0 };
        let (mut state, players, start) = (SceneState::new(spec, power), HashMap::new(), Instant::now());
        let at = |s: u64| start + Duration::from_secs(s);
        for s in [0, 2] {
            state.render(&renderer, ((16, 9), 60), power, &players, true, at(s)).unwrap();
        }
        let time = state.runtime.time();
        assert!(time > 1.0 && state.update(turned));
        state.render(&renderer, ((16, 9), 60), power, &players, true, at(3)).unwrap();
        assert!(state.view.is_some() && state.runtime.time() > time, "the scene restarted");
    }

    #[test]
    fn declared_inputs_without_scripts_wait_for_what_the_scene_binds() {
        let scene = Scene::from_json(include_str!("../../../../scenes/dusk/scene.json")).unwrap();
        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../../../../scenes/dusk/manifest.json")).unwrap();
        let properties = serde_json::from_value(manifest["properties"].clone()).unwrap();
        let mut pack = ScenePack::bare(scene, properties, Vec::new());
        pack.inputs = Some(Uses { cursor: true, scripts: true, ..Uses::default() });
        let pack = Arc::new(pack);
        let power = Power { tier: Tier::T3, on_battery: false, max_fps: 0 };
        let follows = |depth: &str| {
            let mut values = pack.props.values(&[]);
            values.insert("depth".into(), serde_json::json!(depth));
            let spec =
                SceneSpec { values, pack: pack.clone(), extra: Vec::new(), clips: Vec::new(), hold: None };
            let mut state = SceneState::new(spec, power);
            let pointer = PointerState { pos: Some([0.5, 0.5]), inside: true, ..PointerState::default() };
            let moved = state.input(SceneInput::Pointer(String::new(), pointer));
            (state.uses().cursor, state.uses().audio, moved)
        };
        assert_eq!(follows("pointer"), (true, false, true));
        assert_eq!(follows("orbit"), (false, false, false));
        assert_eq!(follows("off"), (false, false, false));
    }
}
