mod level;
mod polling;
#[cfg(test)]
mod tests;

use super::power::Power;
use super::scripts::Link;
use super::slot::PlayerSlot;
use super::{Reel, SceneInput, SceneSpec};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use wprs_core::Region;
use wprs_render::{GpuImage, RenderError, Renderer, SceneView};
use wprs_scene::{
    Course, Fired, Governor, LayerRead, MAX_PRESSES, PointerState, Press, Quality, Runtime, Spare, Touch,
    Uses, Values, Voice, Wake, Why,
};

const CALM: u32 = 30;
const SETTLE: u32 = 3;
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
    presses: Vec<Press>,
    allowed: [bool; 2],
    touches: Vec<Touch>,
    host: Option<Link>,
    calm: u32,
    answered: bool,
    awaiting: u64,
    timer: f64,
    sent: (Values, [f32; 4], [u32; 2], u32),
    read_back: Vec<(String, LayerRead)>,
    sounding: Vec<String>,
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
    pub(super) hushed: bool,
    pub(super) quiet: bool,
    guard: Option<Box<super::guard::Guard>>,
    pub(super) damage: Option<Region>,
    spare: Spare,
    governor: Governor,
    why: Why,
    samples: u64,
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
        let (reads, (quality, why)) = (reads, power.level(&runtime));
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
            presses: Vec::new(),
            allowed: [false; 2],
            touches: Vec::new(),
            host: None,
            calm,
            answered: false,
            awaiting: 0,
            timer: f64::INFINITY,
            sent: (Values::new(), [0.0; 4], [1, 1], 60),
            read_back: Vec::new(),
            sounding: Vec::new(),
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
            quiet: false,
            guard: None,
            damage: None,
            spare: Spare::default(),
            governor: Governor::default(),
            why,
            samples: 0,
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
    pub(super) fn shares(&self) -> (usize, usize) {
        self.host.as_ref().map_or((0, 0), Link::shares)
    }

    pub(super) fn calm(&mut self, asked: bool) -> bool {
        self.quiet = asked;
        let calm = asked && !self.spec.moving;
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
        let moved = spec.moving != self.spec.moving;
        (self.spec, self.uses) = (spec, self.needs(self.quality));
        changed |= moved && self.calm(self.quiet);
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
            SceneInput::Pointer(_, pointer, presses, allowed) => {
                let moved = std::mem::replace(&mut self.pointer, pointer) != pointer;
                let room = MAX_PRESSES.saturating_sub(self.presses.len());
                let pressed = !presses.is_empty();
                self.presses.extend(presses.into_iter().take(room));
                self.allowed = allowed;
                (moved || pressed) && self.uses.cursor
            }
            SceneInput::Script(_, outputs) => {
                let changed = self.runtime.set_script(*outputs);
                self.calm = if changed { 0 } else { self.calm };
                changed
            }
        }
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
        let (quality, uses) = (self.level(power), self.uses);
        if quality != self.quality {
            (self.quality, self.uses) = (quality, self.needs(quality));
        }
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
        let inner = view.inner(renderer, size, quality);
        let rect = view.view(inner);
        let presses = std::mem::take(&mut self.presses);
        if self.uses.cursor && self.runtime.point(view.hitboxes(), rect, &presses, self.allowed.into()) {
            self.calm = 0;
            let room = wprs_scene::MAX_TOUCHES.saturating_sub(self.touches.len());
            self.touches.extend(self.runtime.take_touches().into_iter().take(room));
        }
        let frame = self.runtime.frame_in(std::mem::take(&mut self.spare), rect, quality);
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
        self.damage = wprs_core::damage::join(self.damage, view.damage().map(|r| r.scaled(inner, size)));
        if let Some(guard) = self.guard.as_mut() {
            let image = view.image().filter(|_| view.stats().damage > 0);
            guard.watch(renderer, image, now, &self.spec.pack.props.id)?;
        }
        let (fps, timed) = (power.fps(&self.runtime, quality, refresh), view.gpu_time());
        let interval = Duration::from_secs(1) / fps;
        self.sent = (std::mem::take(&mut self.sent.0), frame.view, [size.0, size.1], fps);
        let stopped = self.guard.as_ref().is_some_and(|g| g.speed() == 0.0);
        let busy = self.runtime.script_busy();
        let timer = match self.timer - self.runtime.time() {
            left if left.is_finite() => Wake::At(left.max(0.0)),
            _ => Wake::Idle,
        };
        let wake = match &self.host {
            _ if self.still || stopped => Wake::Idle,
            Some(h) if !self.answered || h.progress().1 < self.awaiting => Wake::Frame,
            Some(_) if self.calm < if busy { CALM } else { SETTLE } => Wake::Frame,
            Some(_) if busy => frame.wake.min(Wake::At(POLL)).min(timer),
            Some(_) => frame.wake.min(timer),
            None => frame.wake,
        };
        let wake = if self.hushed { self.runtime.calmed(wake) } else { wake };
        self.spare = frame.spare();
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
        self.judge(timed, power, refresh, now);
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
