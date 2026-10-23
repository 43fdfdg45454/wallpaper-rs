use crate::eval::{Frame, walk};
use crate::input::{AudioFrame, PointerState, ScriptOutputs, TimelineCommand};
use crate::particles::{Swarm, plan};
use crate::resolve::Context;
use crate::timeline::Playhead;
use crate::uses::{self, Uses};
use crate::{Affine, Layer, Quality, Scene, Timeline, Values, Voice, Wake};
use std::cell::{Cell, Ref, RefCell};
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

const MAX_STEP: f64 = 0.1;
const MAX_GAP: f64 = 600.0;

pub struct Runtime {
    pub scene: Arc<Scene>,
    props: Values,
    audio: AudioFrame,
    pointer: PointerState,
    script: ScriptOutputs,
    wall: f64,
    t: f64,
    dt: f32,
    stirred: [f64; 2],
    beat: f64,
    timelines: Vec<(Timeline, Playhead)>,
    mattes: BTreeSet<String>,
    memo: RefCell<HashMap<usize, [f32; 4]>>,
    wake: Cell<Wake>,
    uses: [Uses; 4],
    swarms: RefCell<Vec<Swarm>>,
}

impl Runtime {
    pub fn new(scene: Arc<Scene>, props: Values) -> Runtime {
        let timelines = scene.timelines.iter().map(|tl| (tl.clone(), tl.start())).collect();
        let uses = uses::of(&scene, &props);
        let targets = scene.layers().filter_map(|l| l.mask.as_ref()?.layer.clone()).collect();
        let mut mattes = BTreeSet::new();
        holds(&scene.layers, &targets, &mut mattes);
        let (audio, pointer, script, memo) = Default::default();
        let wake = Cell::new(Wake::Frame);
        let (seed, size) = (scene.clock.seed, scene.canvas.size);
        let swarms = RefCell::new(scene.particles.values().map(|s| Swarm::new(s, seed, size)).collect());
        Runtime {
            scene,
            props,
            audio,
            pointer,
            script,
            wall: 0.0,
            t: 0.0,
            dt: 0.0,
            stirred: [f64::NEG_INFINITY; 2],
            beat: f64::NEG_INFINITY,
            timelines,
            mattes,
            memo,
            wake,
            uses,
            swarms,
        }
    }

    pub fn uses(&self) -> Uses {
        self.uses_at(Quality::Q3)
    }

    pub fn uses_at(&self, quality: Quality) -> Uses {
        self.uses[quality as usize]
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
        while self.t < t {
            self.advance((t - self.t).min(MAX_STEP));
        }
    }

    pub fn restart_particles(&mut self) {
        self.swarms.get_mut().iter_mut().for_each(Swarm::restart);
    }

    pub fn advance(&mut self, dt: f64) {
        let mut left = dt.clamp(0.0, MAX_GAP);
        loop {
            let step = left.min(MAX_STEP);
            (self.t, self.dt, left) = (self.t + step, step as f32, left - step);
            self.timelines.iter_mut().for_each(|(tl, head)| tl.advance(head, step as f32));
            if left <= 0.0 {
                return;
            }
        }
    }

    pub fn set_wall(&mut self, seconds: f64) {
        self.wall = seconds;
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
        self.pointer = pointer;
        changed
    }

    pub fn set_script(&mut self, outputs: ScriptOutputs) -> bool {
        for (id, command) in &outputs.timelines {
            let Some((tl, head)) = self.timelines.iter_mut().find(|(tl, _)| &tl.id == id) else { continue };
            match *command {
                TimelineCommand::Play => (head.playing, head.reversed) = (true, false),
                TimelineCommand::Pause => head.playing = false,
                TimelineCommand::Seek(t) => head.t = t.clamp(0.0, tl.duration),
                TimelineCommand::Reverse => (head.playing, head.reversed) = (true, !head.reversed),
            }
        }
        let changed = outputs != self.script;
        self.script = outputs;
        changed
    }

    fn trigger(&mut self, matches: impl Fn(&str) -> bool) {
        let fired = self.timelines.iter_mut().filter(|(tl, _)| tl.trigger.as_deref().is_some_and(&matches));
        fired.for_each(|(tl, head)| *head = Playhead { t: -tl.delay, playing: true, reversed: false });
    }

    pub fn voices(&self) -> (Vec<Voice>, Wake) {
        let cx = self.context();
        let voices = self.scene.sounds().map(|s| s.voice(&cx)).collect();
        (voices, cx.wake.get())
    }

    fn context(&self) -> Context<'_> {
        Context {
            t: self.t as f32,
            dt: self.dt,
            calm: self.stirred.map(|s| (self.t - s) as f32),
            beat: self.since_beat(),
            wall: self.wall,
            audio: &self.audio,
            pointer: &self.pointer,
            script: &self.script,
            props: &self.props,
            timelines: &self.timelines,
            mattes: &self.mattes,
            memo: &self.memo,
            wake: Cell::new(Wake::Idle),
        }
    }

    pub fn frame(&self, view: [f32; 4], quality: Quality) -> Frame<'_> {
        let cx = self.context();
        let canvas = self.scene.canvas.size.map(|v| v as f32);
        let (draws, effects, background) = (Vec::new(), Vec::new(), self.scene.canvas.background);
        let (time, wake) = (self.t as f32, Wake::Idle);
        let mut frame = Frame { draws, effects, time, view, background, quality, wake };
        walk(&cx, &self.scene.layers, Affine::IDENTITY, quality, &mut frame, canvas);
        plan(&self.swarms, &self.scene, &cx, &frame, quality, self.t);
        frame.wake = cx.wake.get();
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
