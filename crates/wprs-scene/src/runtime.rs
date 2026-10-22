use crate::eval::{Affine, Frame, Wake, walk};
use crate::input::{AudioFrame, PointerState, ScriptOutputs, TimelineCommand};
use crate::resolve::Context;
use crate::timeline::Playhead;
use crate::{Kind, Layer, Quality, Scene, Timeline, Use, Values, descriptor};
use serde_json::Value as Json;
use std::cell::{Cell, RefCell};
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

const MAX_STEP: f64 = 0.1;
const MAX_GAP: f64 = 600.0;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Uses {
    pub audio: bool,
    pub cursor: bool,
    pub scripts: bool,
}

pub struct Runtime {
    pub scene: Arc<Scene>,
    props: Values,
    audio: AudioFrame,
    pointer: PointerState,
    script: ScriptOutputs,
    wall: f64,
    t: f64,
    dt: f32,
    timelines: Vec<(Timeline, Playhead)>,
    mattes: BTreeSet<String>,
    memo: RefCell<HashMap<usize, [f32; 4]>>,
    wake: Cell<Wake>,
    uses: Uses,
}

impl Runtime {
    pub fn new(scene: Arc<Scene>, props: Values) -> Runtime {
        let timelines = scene.timelines.iter().map(|tl| (tl.clone(), tl.start())).collect();
        let mut json = serde_json::to_value(&*scene).unwrap_or_default();
        json["ext"] = Json::Null;
        let effect = |u: Use| {
            scene
                .layers()
                .flat_map(|l| &l.effects)
                .any(|e| descriptor(&e.effect).is_some_and(|d| d.uses.contains(&u)))
        };
        let listens = scene.layers().any(|l| matches!(l.kind, Kind::Spectrum(_)))
            || scene.timelines.iter().any(|t| t.trigger.as_deref() == Some("beat"));
        let parallax = scene.layers().any(|l| l.parallax != 0.0);
        let uses = Uses {
            audio: listens || has_key(&json, "audio") || effect(Use::Audio),
            cursor: parallax || has_key(&json, "cursor") || effect(Use::Cursor),
            scripts: !scene.scripts.is_empty(),
        };
        let targets = scene.layers().filter_map(|l| l.mask.as_ref()?.layer.clone()).collect();
        let mut mattes = BTreeSet::new();
        holds(&scene.layers, &targets, &mut mattes);
        let (audio, pointer, script, memo) = Default::default();
        let wake = Cell::new(Wake::Frame);
        Runtime {
            scene,
            props,
            audio,
            pointer,
            script,
            wall: 0.0,
            t: 0.0,
            dt: 0.0,
            timelines,
            mattes,
            memo,
            wake,
            uses,
        }
    }

    pub fn uses(&self) -> Uses {
        self.uses
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

    pub fn seek(&mut self, t: f64) {
        self.timelines.iter_mut().for_each(|(tl, head)| *head = tl.start());
        self.t = 0.0;
        while self.t < t {
            self.advance((t - self.t).min(MAX_STEP));
        }
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
        self.trigger(|t| changed.iter().any(|id| t.strip_prefix("prop:") == Some(id)));
        !changed.is_empty()
    }

    pub fn set_audio(&mut self, audio: AudioFrame) -> bool {
        let beat = audio.beat && !self.audio.beat;
        let changed = self.uses.audio && audio != self.audio && !(audio.silent && self.audio.silent);
        self.audio = audio;
        if beat {
            self.trigger(|t| t == "beat");
        }
        changed
    }

    pub fn set_pointer(&mut self, pointer: PointerState) -> bool {
        let changed = self.uses.cursor && pointer != self.pointer;
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

    pub fn frame(&self, view: [f32; 4], quality: Quality) -> Frame<'_> {
        let cx = Context {
            t: self.t as f32,
            dt: self.dt,
            wall: self.wall,
            audio: &self.audio,
            pointer: &self.pointer,
            script: &self.script,
            props: &self.props,
            timelines: &self.timelines,
            mattes: &self.mattes,
            memo: &self.memo,
            wake: Cell::new(Wake::Idle),
        };
        let canvas = self.scene.canvas.size.map(|v| v as f32);
        let (draws, effects, background) = (Vec::new(), Vec::new(), self.scene.canvas.background);
        let mut frame = Frame { draws, effects, time: self.t as f32, view, background, wake: Wake::Idle };
        walk(&cx, &self.scene.layers, Affine::IDENTITY, quality, &mut frame, canvas);
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

fn has_key(json: &Json, key: &str) -> bool {
    match json {
        Json::Object(map) => map.contains_key(key) || map.values().any(|v| has_key(v, key)),
        Json::Array(items) => items.iter().any(|v| has_key(v, key)),
        _ => false,
    }
}
