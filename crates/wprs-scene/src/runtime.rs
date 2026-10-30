use crate::cue::Fired;
use crate::eval::{Frame, walk};
use crate::input::{AudioFrame, PointerState, ScriptOutputs, TimelineCommand};
use crate::particles::{Swarm, plan};
use crate::resolve::Context;
use crate::timeline::Playhead;
use crate::uses::{self, Uses};
use crate::{Affine, Layer, Quality, Scene, Timeline, Values, Wake};
use std::cell::{Cell, Ref, RefCell};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

const MAX_STEP: f64 = 0.1;
const MAX_GAP: f64 = 600.0;

pub struct Runtime {
    pub scene: Arc<Scene>,
    pub seed: u64,
    props: Values,
    audio: AudioFrame,
    pointer: PointerState,
    glide: PointerState,
    glided: f64,
    tilt: [f32; 2],
    still: bool,
    homes: HashMap<String, String>,
    sheets: BTreeMap<String, Vec<f32>>,
    script: ScriptOutputs,
    wall: f64,
    lang: String,
    t: f64,
    dt: f32,
    stirred: [f64; 2],
    beat: f64,
    pub(crate) timelines: Vec<(Timeline, Playhead)>,
    pub(crate) fired: Vec<Fired>,
    pub(crate) listened: BTreeSet<String>,
    pub(crate) onsets: Vec<Option<f64>>,
    pub(crate) gates: RefCell<Vec<Option<f64>>>,
    pub(crate) lists: Vec<crate::Playlist>,
    mattes: BTreeSet<String>,
    playing: BTreeSet<String>,
    memo: RefCell<HashMap<usize, [f32; 4]>>,
    places: RefCell<HashMap<usize, crate::kin::Place>>,
    motion: RefCell<HashMap<usize, crate::resolve::Motion>>,
    wake: Cell<Wake>,
    pace: Cell<u32>,
    sizes: Cell<(usize, usize)>,
    costs: Cell<(usize, [Option<f32>; 4])>,
    uses: [Uses; 4],
    limit: Option<Uses>,
    swarms: RefCell<Vec<Swarm>>,
}

impl Runtime {
    pub fn new(scene: Arc<Scene>, props: Values) -> Runtime {
        let timelines = scene.timelines.iter().map(|tl| (tl.clone(), tl.start())).collect();
        let uses = uses::of(&scene, &props);
        let targets = scene.layers().flat_map(|l| {
            let masks = l.mask.iter().chain(l.effects.iter().filter_map(|e| e.mask.as_ref()));
            let fed =
                l.effects.iter().flat_map(|e| e.params.values()).filter_map(|v| v.binding()?.layer.clone());
            masks.filter_map(|m| m.layer.clone()).chain(fed)
        });
        let targets = targets.collect();
        let mut mattes = BTreeSet::new();
        holds(&scene.layers, &targets, &mut mattes);
        let (audio, pointer, script, memo) = Default::default();
        let wake = Cell::new(Wake::Frame);
        let fresh = || std::time::UNIX_EPOCH.elapsed().map_or(0, |d| d.as_nanos() as u64 ^ d.as_secs() << 32);
        let (seed, size) = (scene.clock.seed.unwrap_or_else(fresh), scene.canvas.size);
        let homes = crate::kin::homes(&scene);
        let swarms = RefCell::new(scene.particles.values().map(|s| Swarm::new(s, seed, size)).collect());
        let listened = crate::voice::listened(&scene, true, true);
        let onsets = crate::voice::onsets(&scene);
        let gates = RefCell::new(vec![None; onsets.len()]);
        let mut runtime = Runtime {
            scene,
            seed,
            props,
            audio,
            pointer,
            glide: pointer,
            glided: 0.0,
            tilt: [0.0; 2],
            still: false,
            homes,
            sheets: BTreeMap::new(),
            script,
            wall: 0.0,
            lang: String::new(),
            t: 0.0,
            dt: 0.0,
            stirred: [f64::NEG_INFINITY; 2],
            beat: f64::NEG_INFINITY,
            timelines,
            fired: Vec::new(),
            listened,
            onsets,
            gates,
            lists: Vec::new(),
            mattes,
            playing: BTreeSet::new(),
            memo,
            places: RefCell::default(),
            motion: RefCell::default(),
            wake,
            pace: Cell::new(u32::MAX),
            sizes: Cell::default(),
            costs: Cell::default(),
            uses,
            limit: None,
            swarms,
        };
        runtime.set_lengths(BTreeMap::new());
        runtime
    }

    pub fn declare(&mut self, allowed: Uses) {
        self.limit = Some(allowed);
    }

    pub fn uses(&self) -> Uses {
        self.uses_at(Quality::Q3)
    }

    pub fn uses_at(&self, quality: Quality) -> Uses {
        let found = self.uses[quality as usize];
        let Some(limit) = self.limit else { return found };
        let (audio, cursor, clicks) =
            (found.audio && limit.audio, found.cursor && limit.cursor, found.clicks && limit.clicks);
        Uses { audio, cursor, clicks, media: found.media && limit.media, ..found }
    }

    pub fn since_beat(&self) -> f32 {
        (self.t - self.beat).min(f64::from(f32::MAX)) as f32
    }

    pub fn time(&self) -> f64 {
        self.t
    }

    pub fn props(&self) -> &Values {
        &self.props
    }

    pub fn audio(&self) -> &AudioFrame {
        &self.audio
    }

    pub fn pointer(&self) -> &PointerState {
        &self.pointer
    }

    pub fn dt(&self) -> f32 {
        self.dt
    }

    pub fn cost(&self, quality: Quality) -> f32 {
        let at = Arc::as_ptr(&self.scene) as usize;
        let (kept, mut costs) = self.costs.get();
        costs = if kept == at { costs } else { [None; 4] };
        let cost = *costs[quality as usize].get_or_insert_with(|| self.scene.cost(quality));
        self.costs.set((at, costs));
        cost
    }

    pub fn calmed(&self, wake: Wake) -> Wake {
        let minute = 60.0 - self.wall.rem_euclid(60.0);
        match wake {
            Wake::Frame => Wake::At(minute),
            Wake::At(at) => Wake::At(at.max(minute)),
            Wake::Idle => Wake::Idle,
        }
    }

    pub fn pace(&self) -> u32 {
        self.pace.get().max(1)
    }

    pub fn wake(&self) -> Wake {
        self.wake.get()
    }

    pub fn swarm(&self, system: usize) -> Option<Ref<'_, Swarm>> {
        Ref::filter_map(self.swarms.borrow(), |s| s.get(system)).ok()
    }

    pub fn seek(&mut self, t: f64) {
        self.timelines.iter_mut().for_each(|(tl, head)| *head = tl.start());
        (self.t, self.stirred, self.beat) = (0.0, [f64::NEG_INFINITY; 2], f64::NEG_INFINITY);
        self.swarms.get_mut().iter_mut().for_each(Swarm::restart);
        (self.onsets, self.gates) =
            (crate::voice::onsets(&self.scene), RefCell::new(vec![None; self.onsets.len()]));
        while self.t < t {
            self.advance((t - self.t).min(MAX_STEP));
        }
        self.fired.clear();
    }

    pub fn restart_particles(&mut self) {
        self.swarms.get_mut().iter_mut().for_each(Swarm::restart);
    }

    pub fn advance(&mut self, dt: f64) {
        let mut left = dt.clamp(0.0, MAX_GAP);
        loop {
            let step = left.min(MAX_STEP);
            (self.t, self.dt, left) = (self.t + step, step as f32, left - step);
            self.tick(step as f32);
            if left <= 0.0 {
                return;
            }
        }
    }

    pub fn set_wall(&mut self, local: f64) {
        self.wall = local;
    }

    pub fn wall(&self) -> f64 {
        self.wall
    }

    pub fn parallax(&self) -> [f32; 2] {
        let glide = self.scene.canvas.parallax.clone().unwrap_or_default();
        let tilt = if glide.gyro && !self.still { self.tilt } else { [0.0; 2] };
        [0, 1].map(|i| ((self.glide.smooth[i] - 0.5) * glide.pointer + tilt[i] * 0.5) * glide.amount)
    }

    pub fn set_language(&mut self, lang: &str) {
        self.lang = lang.to_owned();
    }

    pub fn set_props(&mut self, props: Values) -> bool {
        let changed: Vec<String> =
            props.iter().filter(|(k, v)| self.props.get(*k) != Some(*v)).map(|(k, _)| k.clone()).collect();
        self.props = props;
        if !changed.is_empty() {
            self.uses = uses::of(&self.scene, &self.props);
        }
        self.trigger(|t| changed.iter().any(|id| t.strip_prefix("prop:") == Some(id)));
        !changed.is_empty()
    }

    pub fn set_audio(&mut self, audio: AudioFrame) -> bool {
        let beat = audio.beat && !self.audio.beat;
        let changed = self.uses().audio && audio != self.audio && !(audio.silent && self.audio.silent);
        if !audio.silent {
            self.stirred[1] = self.t;
        }
        self.audio = audio;
        if beat {
            self.beat = self.t;
            self.trigger(|t| t == "beat");
        }
        changed
    }

    pub fn set_pointer(&mut self, pointer: PointerState) -> bool {
        let changed = self.uses().cursor && pointer != self.pointer;
        if pointer.moving() || pointer.buttons & !self.pointer.buttons != 0 {
            self.stirred[0] = self.t;
        }
        let delay = self.scene.canvas.parallax.as_ref().map_or(crate::input::SETTLE, |g| g.delay);
        let dt = (self.t - self.glided).clamp(0.0, MAX_STEP) as f32;
        self.glide = match delay == crate::input::SETTLE {
            true => pointer,
            false => self.glide.follow_in(pointer.pos, pointer.buttons, dt, delay),
        };
        (self.pointer, self.glided) = (pointer, self.t);
        changed
    }

    pub fn set_tilt(&mut self, tilt: [f32; 2]) -> bool {
        let tilt = tilt.map(|v| v.clamp(-1.0, 1.0));
        let changed =
            !self.still && self.scene.tilts() && (0..2).any(|i| (tilt[i] - self.tilt[i]).abs() > 1e-4);
        self.tilt = tilt;
        changed
    }

    pub fn set_reduced_motion(&mut self, still: bool) -> bool {
        let moves = self.scene.layers().any(|l| !l.parallax.is_zero());
        moves && std::mem::replace(&mut self.still, still) != still
    }

    pub fn set_sheet(&mut self, asset: &str, durations: Vec<f32>) {
        self.sheets.insert(asset.to_owned(), durations);
    }

    pub fn set_script(&mut self, outputs: ScriptOutputs) -> bool {
        for (id, command) in &outputs.timelines {
            let Some((tl, head)) = self.timelines.iter_mut().find(|(tl, _)| &tl.id == id) else { continue };
            match *command {
                TimelineCommand::Play => (head.playing, head.reversed) = (true, false),
                TimelineCommand::Pause => head.playing = false,
                TimelineCommand::Seek(t) => head.t = t.clamp(0.0, tl.length()),
                TimelineCommand::Reverse => (head.playing, head.reversed) = (true, !head.reversed),
                TimelineCommand::Stop => (head.t, head.playing, head.reversed) = (0.0, false, false),
                TimelineCommand::Speed(rate) => head.rate = rate.clamp(-100.0, 100.0),
            }
        }
        let changed = outputs != self.script;
        self.script = outputs;
        changed
    }

    pub fn set_playing(&mut self, assets: BTreeSet<String>) {
        self.playing = assets;
    }

    pub fn press(&mut self, id: &str) {
        self.trigger(|t| t.strip_prefix("prop:") == Some(id));
    }

    fn trigger(&mut self, matches: impl Fn(&str) -> bool) {
        self.trigger_late(matches, 0.0);
    }

    pub(crate) fn trigger_late(&mut self, matches: impl Fn(&str) -> bool, late: f32) -> Vec<usize> {
        let mut started = Vec::new();
        for (i, (tl, head)) in self.timelines.iter_mut().enumerate() {
            if tl.trigger.as_deref().is_some_and(&matches) {
                *head = Playhead {
                    t: tl.origin(head.rate) + late * head.rate,
                    playing: true,
                    reversed: false,
                    ..*head
                };
                started.push(i);
            }
        }
        let t = self.t - f64::from(late);
        let sounds = self.scene.sounds().map(|s| s.play_on.as_deref().is_some_and(&matches));
        self.onsets.iter_mut().zip(sounds).filter(|(_, on)| *on).for_each(|(onset, _)| *onset = Some(t));
        started
    }

    pub(crate) fn context(&self) -> Context<'_> {
        Context {
            t: self.t as f32,
            dt: self.dt,
            calm: self.stirred.map(|s| (self.t - s) as f32),
            beat: self.since_beat(),
            wall: self.wall,
            step: f64::from(self.scene.clock.step.unwrap_or(60.0)).max(1.0),
            lang: &self.lang,
            audio: &self.audio,
            pointer: &self.pointer,
            script: &self.script,
            props: &self.props,
            timelines: &self.timelines,
            mattes: &self.mattes,
            playing: &self.playing,
            memo: &self.memo,
            wake: Cell::new(Wake::Idle),
            pace: Cell::new(0),
            scene: &self.scene,
            homes: &self.homes,
            places: {
                self.places.borrow_mut().clear();
                &self.places
            },
            motion: &self.motion,
            now: self.t,
            view: [0.0; 4],
            canvas: self.scene.canvas.size.map(|v| v as f32),
            glide: &self.glide,
            tilt: if self.still { [0.0; 2] } else { self.tilt },
            still: self.still,
            sheets: &self.sheets,
        }
    }

    pub fn frame(&self, view: [f32; 4], quality: Quality) -> Frame<'_> {
        let cx = Context { view, ..self.context() };
        let canvas = self.scene.canvas.size.map(|v| v as f32);
        let (draws, effects) =
            (Vec::with_capacity(self.sizes.get().0), Vec::with_capacity(self.sizes.get().1));
        let background = self.scene.canvas.background;
        let (time, wake) = (self.t as f32, Wake::Idle);
        let mut frame = Frame { draws, effects, time, view, background, quality, wake };
        walk(&cx, &self.scene.layers, Affine::IDENTITY, quality, &mut frame, canvas);
        plan(&self.swarms, &self.scene, &cx, &frame, quality, self.t);
        frame.wake = cx.wake.get().min(self.cue_wake());
        self.pace.set(if frame.wake == Wake::Frame { cx.pace.get() } else { u32::MAX });
        self.sizes.set((frame.draws.len(), frame.effects.len()));
        self.wake.set(frame.wake);
        frame
    }
}

fn holds(layers: &[Layer], targets: &BTreeSet<String>, found: &mut BTreeSet<String>) -> bool {
    let mut any = false;
    for layer in layers {
        if holds(layer.children(), targets, found) || targets.contains(&layer.id) {
            found.insert(layer.id.clone());
            any = true;
        }
    }
    any
}
