use super::power::Power;
use super::scripts::{Link, Scripts};
use super::slot::PlayerSlot;
use super::{Reel, SceneInput, SceneSpec};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::time::{Duration, Instant};
use wprs_render::{GpuImage, RenderError, Renderer, SceneView, log_allowed};
use wprs_scene::{
    Course, Fired, LayerRead, MAX_FIRED, PointerState, Quality, Runtime, ScriptOutputs, Uses, Values, Voice,
    Wake,
};
use wprs_script::{Error, Frame, Mode, Setup};

const CALM: u32 = 30;
const SETTLE: u32 = 3;
const KEPT_LINES: usize = 50;
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
    host: Option<Link>,
    calm: u32,
    answered: bool,
    timer: f64,
    sent: (Values, [f32; 4], [u32; 2], u32),
    pub(super) console: std::collections::VecDeque<String>,
    pub(super) error: Option<String>,
    pub(super) left: Vec<String>,
    left_key: u64,
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
    hushed: bool,
    guard: Option<Box<super::guard::Guard>>,
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
        let exposed = wprs_scene::exposed(&spec.pack.props.properties, &spec.values);
        let [audio, cursor] = wprs_scene::script_inputs(&spec.pack.scene, &spec.pack.scripts);
        let reads = spec.pack.inputs.map_or((audio, cursor), |u| (audio && u.audio, cursor && u.cursor));
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
            host: None,
            calm,
            answered: false,
            timer: f64::INFINITY,
            sent: (Values::new(), [0.0; 4], [1, 1], 60),
            console: Default::default(),
            error,
            left: Vec::new(),
            left_key: 0,
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
            hushed: false,
            guard: None,
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

    pub(super) fn retry(&mut self) {
        if self.view.as_mut().is_some_and(SceneView::retry) {
            self.view = None;
        }
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
        if !self.still && !self.hushed {
            self.runtime.seek(time);
        }
    }

    pub(super) fn uses(&self) -> Uses {
        self.uses
    }

    pub(super) fn playing(&self) -> bool {
        self.moving || self.host.is_some()
    }

    #[cfg(test)]
    pub(super) fn shares(&self) -> usize {
        self.host.as_ref().map_or(0, Link::shares)
    }

    pub(super) fn calm(&mut self, calm: bool) -> bool {
        let parallax = self.runtime.set_reduced_motion(calm);
        if std::mem::replace(&mut self.hushed, calm) == calm {
            return parallax;
        }
        if calm && !self.still {
            self.runtime.seek(f64::from(self.runtime.scene.clock.poster_time));
        }
        true
    }

    pub(super) fn guard(&mut self, on: bool) -> bool {
        let changed = on != self.guard.is_some();
        if changed {
            self.guard = on.then(|| super::guard::Guard::new(Instant::now()));
        }
        changed
    }

    pub(super) fn particles(&self) -> &'static str {
        match self.view.as_ref().and_then(SceneView::gpu_particles) {
            Some(true) => "gpu",
            Some(false) => "cpu",
            None => "",
        }
    }

    pub(super) fn flashes(&self) -> &'static str {
        self.guard.as_ref().map_or("", |g| g.label())
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
        if changed {
            self.calm = 0;
        }
        changed || media
    }

    pub(super) fn input(&mut self, input: SceneInput) -> bool {
        if self.guard.as_ref().is_some_and(|g| g.speed() == 0.0) {
            return false;
        }
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
                let changed = self.runtime.set_script(*outputs);
                self.calm = if changed { 0 } else { self.calm };
                changed
            }
        }
    }

    pub(super) fn link(&mut self, scripts: &mut Scripts) {
        if self.still || !self.runtime.uses().scripts || self.host.is_some() {
            return;
        }
        let pack = &self.spec.pack;
        let key = (Arc::as_ptr(pack) as usize, serde_json::to_string(&self.spec.values).unwrap_or_default());
        let named = (pack.locale.as_str(), &pack.labels);
        let setup = || Setup::new(&pack.scene, &pack.scripts, self.runtime.seed, named);
        self.host = Some(super::scripts::link(scripts, key, setup));
    }

    pub(super) fn script(&mut self, output: &str, mode: Mode) -> Option<ScriptOutputs> {
        let fresh = self.runtime.take_fired();
        let room = MAX_FIRED.saturating_sub(self.fired.len());
        self.fired.extend(fresh.into_iter().take(room));
        let rt = &self.runtime;
        rt.fill_playbacks(&mut self.courses);
        let minute = rt.wall().div_euclid(60.0) as i64;
        if self.zone.0 != minute {
            self.zone = (minute, crate::zone::posix());
        }
        let reads: Vec<(String, LayerRead)> =
            rt.script_watch().iter().filter_map(|id| Some((id.clone(), rt.read(id)?))).collect();
        let actions = std::mem::take(&mut self.pressed);
        let (view, screen, fps) = (self.sent.1, self.sent.2, self.sent.3);
        let eventful = !self.fired.is_empty()
            || !actions.is_empty()
            || self.timer <= rt.time()
            || self.sent.0 != self.exposed;
        let audio = (self.reads.0 && self.uses.audio).then(|| rt.audio());
        let frame = || Frame {
            time: rt.time(),
            dt: rt.dt(),
            wall: rt.wall(),
            zone: self.zone.1.as_str(),
            props: &self.exposed,
            actions: &actions,
            audio,
            pointer: rt.pointer(),
            view,
            screen,
            mode,
            fps,
            quality: self.quality as u8,
            timelines: &self.courses,
            fired: &self.fired,
            reads: &reads,
            touches: &[],
        };
        let inside = rt.pointer().inside;
        let sent = self.host.as_mut()?.poll(output, inside, frame);
        if eventful {
            self.calm = 0;
            self.sent.0 = self.exposed.clone();
        }
        self.fired.clear();
        match sent {
            Ok(Some(mut outputs)) => {
                self.answered = true;
                self.heard_script(&mut outputs);
                Some(outputs)
            }
            Ok(None) => None,
            Err(Error::Disabled) => {
                tracing::warn!(
                    scene = self.spec.pack.props.id,
                    "the scene scripts failed too often and are off"
                );
                (self.host, self.error) = (None, Some(crate::words::scripts_off()));
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

    fn heard_script(&mut self, outputs: &mut ScriptOutputs) {
        for line in outputs.console.drain(..) {
            tracing::debug!(scene = self.spec.pack.props.id, "script {line}");
            self.console.push_back(line);
        }
        while self.console.len() > KEPT_LINES {
            self.console.pop_front();
        }
        let stopped = outputs.stopped.last().map(|why| crate::words::script_stopped(why));
        if stopped.is_some() && stopped != self.error {
            tracing::warn!(
                scene = self.spec.pack.props.id,
                "a scene script stopped: {}",
                outputs.stopped.join("; ")
            );
            self.error = stopped;
        }
        self.timer = outputs.wake.map_or(f64::INFINITY, |w| self.runtime.time() + w);
    }

    pub(super) fn clean(&self) -> bool {
        self.view.as_ref().is_some_and(|v| v.stats().damage == 0)
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
        let rendered = renderer.render_scene(view, &self.runtime, &frame, inner, quality, &videos).map(drop);
        if view.left_key() != self.left_key {
            (self.left, self.left_key) = (view.left_out(), view.left_key());
        }
        rendered?;
        if let Some(guard) = self.guard.as_mut() {
            let image = view.image().filter(|_| view.stats().damage > 0);
            guard.watch(renderer, image, now, &self.spec.pack.props.id)?;
        }
        let fps = power.fps(&self.runtime, quality, refresh);
        let interval = Duration::from_secs(1) / fps;
        self.sent = (std::mem::take(&mut self.sent.0), frame.view, [size.0, size.1], fps);
        let stopped = self.guard.as_ref().is_some_and(|g| g.speed() == 0.0);
        let busy = self.runtime.script_busy();
        let timer = match self.timer - self.runtime.time() {
            left if left.is_finite() => Wake::At(left.max(0.0)),
            _ => Wake::Idle,
        };
        let wake = match self.host {
            _ if self.still || stopped => Wake::Idle,
            Some(_) if !self.answered || self.calm < if busy { CALM } else { SETTLE } => Wake::Frame,
            Some(_) if busy => frame.wake.min(Wake::At(POLL)).min(timer),
            Some(_) => frame.wake.min(timer),
            None => frame.wake,
        };
        let wake = if self.hushed { self.runtime.calmed(wake) } else { wake };
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
        if active && !self.still && !self.hushed {
            self.runtime.advance(dt * self.guard.as_ref().map_or(1.0, |g| g.speed()));
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
        let renderer = match Renderer::shared() {
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
        let extra = vec![("photo".to_string(), wprs_render::Pixels::new(image::RgbaImage::new(2, 2)))];
        let turned = SceneSpec { extra, ..spec.clone() };
        let (old, new) = (Content::Scene(spec.clone()), Content::Scene(turned.clone()));
        assert!(old.same_source(&new) && !old.same_look(&new));
        let power = Power::new(Tier::T3, false, 0);
        let (mut state, players, start) = (SceneState::new(spec, power), HashMap::new(), Instant::now());
        let at = |s: u64| start + Duration::from_secs(s);
        for s in [0, 2] {
            state.render(renderer, ((16, 9), 60), power, &players, true, at(s)).unwrap();
        }
        let time = state.runtime.time();
        assert!(time > 1.0 && state.update(turned));
        state.render(renderer, ((16, 9), 60), power, &players, true, at(3)).unwrap();
        assert!(state.view.is_some() && state.runtime.time() > time, "the scene restarted");
    }

    #[test]
    fn screens_showing_one_scene_share_its_script_process() {
        let scene = Scene::from_json(include_str!("../../../../scenes/orbits/scene.json")).unwrap();
        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../../../../scenes/orbits/manifest.json")).unwrap();
        let properties = serde_json::from_value(manifest["properties"].clone()).unwrap();
        let mut pack = ScenePack::bare(scene, properties, Vec::new());
        pack.scripts = vec![(
            "scripts/orbits.js".into(),
            include_str!("../../../../scenes/orbits/scripts/orbits.js").into(),
        )];
        let pack = Arc::new(pack);
        let power = Power::new(Tier::T3, false, 0);
        let spec = |values: Values| SceneSpec {
            values,
            pack: pack.clone(),
            extra: Vec::new(),
            clips: Vec::new(),
            hold: None,
        };
        let mut scripts = Scripts::default();
        let mut states: Vec<SceneState> =
            (0..3).map(|_| SceneState::new(spec(pack.props.values(&[])), power)).collect();
        let mut other = pack.props.values(&[]);
        other.insert("other".into(), serde_json::json!(1));
        states.push(SceneState::new(spec(other), power));
        states.iter_mut().for_each(|s| s.link(&mut scripts));
        assert_eq!(states.iter().map(SceneState::shares).collect::<Vec<_>>(), [3, 3, 3, 1]);
        states.truncate(1);
        assert_eq!(states[0].shares(), 1);
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
        let power = Power::new(Tier::T3, false, 0);
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
