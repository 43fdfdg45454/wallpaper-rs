use crate::input::{AudioFrame, PointerState, ScriptOutputs};
use crate::kin::Place;
use crate::timeline::{Loop, Playhead, local};
use crate::value::{AudioChannel, Axis, Band, Binding, Level};
use crate::{BANDS, Reduce, Scene, Timeline, Values, Wake};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet, HashMap};

pub(crate) mod binding;

pub(crate) struct Context<'a> {
    pub(crate) t: f32,
    pub(crate) dt: f32,
    pub(crate) calm: [f32; 2],
    pub(crate) beat: f32,
    pub(crate) wall: f64,
    pub(crate) step: f64,
    pub(crate) lang: &'a str,
    pub(crate) audio: &'a AudioFrame,
    pub(crate) pointer: &'a PointerState,
    pub(crate) script: &'a ScriptOutputs,
    pub(crate) props: &'a Values,
    pub(crate) timelines: &'a [(Timeline, Playhead)],
    pub(crate) mattes: &'a BTreeSet<String>,
    pub(crate) playing: &'a BTreeSet<String>,
    pub(crate) memo: &'a RefCell<HashMap<usize, [f32; 4]>>,
    pub(crate) wake: Cell<Wake>,
    pub(crate) pace: Cell<u32>,
    pub(crate) scene: &'a Scene,
    pub(crate) homes: &'a HashMap<String, String>,
    pub(crate) places: &'a RefCell<HashMap<usize, Place>>,
    pub(crate) motion: &'a RefCell<HashMap<usize, Motion>>,
    pub(crate) now: f64,
    pub(crate) view: [f32; 4],
    pub(crate) canvas: [f32; 2],
    pub(crate) camera: crate::Affine,
    pub(crate) glide: &'a PointerState,
    pub(crate) tilt: [f32; 2],
    pub(crate) still: bool,
    pub(crate) sheets: &'a BTreeMap<String, Vec<f32>>,
    pub(crate) flocks: &'a [crate::particles::Flock],
    pub(crate) tapes: &'a [crate::Tape],
}

pub(crate) type Motion = ([f32; 2], f64, [f32; 2]);

impl Context<'_> {
    pub(crate) fn velocity(&self, key: usize, at: [f32; 2]) -> [f32; 2] {
        let mut motion = self.motion.borrow_mut();
        let (last, then, v) = motion.get(&key).copied().unwrap_or((at, self.now, [0.0; 2]));
        let dt = (self.now - then) as f32;
        let v = match dt {
            dt if dt.abs() < 1e-4 => v,
            dt if (0.0..1.0).contains(&dt) => [(at[0] - last[0]) / dt, (at[1] - last[1]) / dt],
            _ => [0.0; 2],
        };
        motion.insert(key, (at, self.now, v));
        v
    }

    pub(crate) fn stirred(&self, key: usize, stirring: bool, settle: f32) -> Wake {
        let mut motion = self.motion.borrow_mut();
        if stirring {
            motion.insert(key + 1, ([0.0; 2], f64::from(self.t), [0.0; 2]));
        }
        let calm = motion.get(&(key + 1)).map_or(f32::INFINITY, |m| self.t - m.1 as f32);
        if (0.0..settle).contains(&calm) { Wake::Frame } else { Wake::Idle }
    }

    pub(crate) fn note(&self, wake: Wake) {
        self.paced(wake, None);
    }

    pub(crate) fn paced(&self, wake: Wake, fps: Option<u32>) {
        if wake == Wake::Frame {
            self.pace.set(self.pace.get().max(fps.unwrap_or(u32::MAX)));
        }
        self.wake.set(self.wake.get().min(wake));
    }

    pub(crate) fn signal(&self, b: &Binding<impl Sized>) -> Option<f32> {
        let a = self.audio;
        let bands = match b.channel.unwrap_or_default() {
            AudioChannel::Average => &a.average,
            AudioChannel::Left => &a.left,
            AudioChannel::Right => &a.right,
        };
        let of = b.of.unwrap_or(BANDS as u32).clamp(1, BANDS as u32) as usize;
        let group = |lo: u32, hi: u32| {
            let a = (lo as usize * BANDS / of).min(BANDS - 1);
            let z = ((hi.max(lo) as usize + 1) * BANDS / of).clamp(a + 1, BANDS);
            let it = bands[a..z].iter().copied();
            match b.reduce.unwrap_or_default() {
                Reduce::Max => it.fold(0.0, f32::max),
                Reduce::Mean => it.sum::<f32>() / (z - a) as f32,
            }
        };
        let audio = b.audio.map(|band| match band {
            Band::Index(i) => group(i, i),
            Band::Range([lo, hi]) => group(lo, hi),
            Band::Named(Level::Bass) => a.bass,
            Band::Named(Level::Mid) => a.mid,
            Band::Named(Level::Treble) => a.treble,
            Band::Named(Level::Level) => a.level,
            Band::Named(Level::Beat) => f32::from(a.beat),
        });
        if audio.is_some() && !a.silent {
            self.note(Wake::Frame);
        }
        let p = self.pointer;
        let cursor = b.cursor.map(|axis| match axis {
            Axis::X => p.smooth[0],
            Axis::Y => p.smooth[1],
            Axis::Inside => f32::from(p.inside),
            Axis::Speed => p.velocity[0].hypot(p.velocity[1]),
        });
        if cursor.is_some() && p.moving() {
            self.note(Wake::Frame);
        }
        audio.or(cursor)
    }

    pub(crate) fn clock(&self, pattern: &str, dates: bool) -> String {
        let (text, wake) = crate::calendar::format(pattern, self.wall, self.lang, dates);
        wake.into_iter().for_each(|at| self.note(Wake::At(at)));
        text
    }

    pub(crate) fn time_of_day(&self, name: &str) -> f32 {
        let (value, at) = crate::calendar::number(name, self.wall, self.step);
        self.note(Wake::At(at));
        value
    }

    pub(crate) fn keys_time(&self, b: &Binding<impl Sized>, end: f32) -> f32 {
        if let Some(id) = &b.timeline {
            let Some((tl, head)) = self.timelines.iter().find(|(tl, _)| &tl.id == id) else { return 0.0 };
            if head.playing {
                self.note(if head.t < 0.0 && head.reversed == (head.rate < 0.0) {
                    Wake::At(f64::from(-head.t / head.rate.abs().max(1e-3)))
                } else {
                    Wake::Frame
                });
            }
            return tl.position(head);
        }
        let repeat = b.repeat.unwrap_or_default();
        if repeat != Loop::Once || self.t < end {
            self.note(Wake::Frame);
        }
        local(self.t, end, repeat)
    }
}
