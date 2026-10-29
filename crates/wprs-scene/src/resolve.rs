use crate::animate::Animate;
use crate::input::{AudioFrame, PointerState, ScriptOutputs};
use crate::kin::Place;
use crate::timeline::{Loop, Playhead, local};
use crate::value::{AudioChannel, Axis, Band, Binding, Level, Value};
use crate::{Scene, Timeline, Values, Wake};
use serde_json::Value as Json;
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet, HashMap};

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
    pub(crate) glide: &'a PointerState,
    pub(crate) tilt: [f32; 2],
    pub(crate) still: bool,
    pub(crate) sheets: &'a BTreeMap<String, Vec<f32>>,
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

    pub(crate) fn note(&self, wake: Wake) {
        self.paced(wake, None);
    }

    pub(crate) fn paced(&self, wake: Wake, fps: Option<u32>) {
        if wake == Wake::Frame {
            self.pace.set(self.pace.get().max(fps.unwrap_or(u32::MAX)));
        }
        self.wake.set(self.wake.get().min(wake));
    }

    fn signal(&self, b: &Binding<impl Sized>) -> Option<f32> {
        let a = self.audio;
        let bands = match b.channel.unwrap_or_default() {
            AudioChannel::Average => &a.average,
            AudioChannel::Left => &a.left,
            AudioChannel::Right => &a.right,
        };
        let audio = b.audio.map(|band| match band {
            Band::Index(i) => bands.get(i as usize).copied().unwrap_or(0.0),
            Band::Named(Level::Bass) => a.bass,
            Band::Named(Level::Mid) => a.mid,
            Band::Named(Level::Treble) => a.treble,
            Band::Named(Level::Level) => a.level,
            Band::Named(Level::Beat) => f32::from(a.beat),
            Band::Range(_) => 0.0,
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

    fn clock(&self, pattern: &str, dates: bool) -> String {
        let (text, wake) = crate::calendar::format(pattern, self.wall, self.lang, dates);
        wake.into_iter().for_each(|at| self.note(Wake::At(at)));
        text
    }

    fn time_of_day(&self, name: &str) -> f32 {
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

impl<T: Animate> Value<T> {
    pub fn settled(&self, props: Option<&Values>) -> Option<T> {
        let b = match self {
            Value::Fixed(v) => return Some(v.clone()),
            Value::Bound(b) => b,
        };
        let id = b.prop.as_ref().filter(|_| b.keys.is_none())?;
        let value = shaped(b, prop(b, id, props?));
        Some(b.base.as_ref().map_or_else(|| value.clone(), |base| added(base, &value)))
    }

    pub(crate) fn get(&self, cx: &Context) -> T {
        let b = match self {
            Value::Fixed(v) => return v.clone(),
            Value::Bound(b) => b,
        };
        let raw = if b.tracked() {
            let base = b.base.clone().unwrap_or_default();
            let (mut parts, _) = base.parts();
            for (name, track) in b.tracks() {
                let i = ["x", "y", "r", "g", "b", "a"].iter().position(|n| *n == name).unwrap_or(0);
                track.iter().for_each(|v| parts[if i < 2 { i } else { i - 2 }] = v.get(cx));
            }
            base.with_parts(parts)
        } else if let Some(keys) = &b.keys {
            crate::cue::wrapped(keys, cx.keys_time(b, keys.last().map_or(0.0, |k| k.t)), cx.wrap(b))
        } else if let Some(id) = &b.prop {
            prop(b, id, cx.props)
        } else if let Some(pattern) = &b.clock {
            match T::from_signal(0.0).is_some() && crate::CLOCKS.contains(&pattern.as_str()) {
                true => T::from_signal(cx.time_of_day(pattern)).unwrap_or_default(),
                false => T::from_text(cx.clock(pattern, false)).unwrap_or_default(),
            }
        } else if let Some(pattern) = &b.date {
            T::from_text(cx.clock(pattern, true)).unwrap_or_default()
        } else if let Some(path) = &b.script {
            cx.script.values.get(path).and_then(T::from_json).unwrap_or_default()
        } else {
            cx.signal(b).and_then(T::from_signal).unwrap_or_default()
        };
        let value = match b.base.as_ref().filter(|_| !b.tracked()) {
            Some(base) => added(base, &shaped(b, raw)),
            None => shaped(b, raw),
        };
        let Some(tau) = b.smooth.and_then(crate::Smooth::seconds) else { return value };
        let (comps, n) = value.comps();
        let mut memo = cx.memo.borrow_mut();
        let last = memo.entry(&**b as *const Binding<T> as usize).or_insert(comps);
        let k = 1.0 - (-cx.dt / tau).exp();
        last[..n].iter_mut().zip(&comps).for_each(|(l, c)| *l += (c - *l) * k);
        if last[..n].iter().zip(&comps).any(|(l, c)| (l - c).abs() > 1e-3) {
            cx.note(Wake::Frame);
        }
        value.with_comps(*last)
    }
}

fn prop<T: Animate>(b: &Binding<T>, id: &str, props: &Values) -> T {
    let value = props.get(id).unwrap_or(&Json::Null);
    let mapped = b.map.as_ref().and_then(|map| map.get(&key(value)).cloned());
    mapped.or_else(|| T::from_json(value)).unwrap_or_default()
}

fn shaped<T: Animate>(b: &Binding<T>, raw: T) -> T {
    let (mut comps, n) = raw.comps();
    comps[..n].iter_mut().for_each(|c| *c = b.modify(*c));
    if n == 0 { raw } else { raw.with_comps(comps) }
}

fn added<T: Animate>(base: &T, value: &T) -> T {
    let ((mut parts, n), (more, _)) = (base.parts(), value.parts());
    parts[..n].iter_mut().zip(more).for_each(|(p, m)| *p += m);
    base.with_parts(parts)
}

pub(crate) fn key(value: &Json) -> String {
    match value {
        Json::String(s) => s.clone(),
        other => other.to_string(),
    }
}
