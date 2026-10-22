use crate::animate::Animate;
use crate::eval::Wake;
use crate::input::{AudioFrame, PointerState, ScriptOutputs};
use crate::timeline::{Loop, Playhead, local, sample};
use crate::value::{AudioChannel, Axis, Band, Binding, Level, Value};
use crate::{Timeline, Values};
use serde_json::Value as Json;
use std::cell::{Cell, RefCell};
use std::collections::{BTreeSet, HashMap};

pub(crate) struct Context<'a> {
    pub(crate) t: f32,
    pub(crate) dt: f32,
    pub(crate) wall: f64,
    pub(crate) audio: &'a AudioFrame,
    pub(crate) pointer: &'a PointerState,
    pub(crate) script: &'a ScriptOutputs,
    pub(crate) props: &'a Values,
    pub(crate) timelines: &'a [(Timeline, Playhead)],
    pub(crate) mattes: &'a BTreeSet<String>,
    pub(crate) memo: &'a RefCell<HashMap<usize, [f32; 4]>>,
    pub(crate) wake: Cell<Wake>,
}

impl Context<'_> {
    pub(crate) fn note(&self, wake: Wake) {
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

    fn clock(&self, pattern: &str) -> String {
        let s = self.wall.rem_euclid(86_400.0) as u32;
        let (h, m, sec) = (s / 3600, s / 60 % 60, s % 60);
        let step = if pattern.contains('s') { 1.0 } else { 60.0 };
        self.note(Wake::At(step - self.wall.rem_euclid(step)));
        let h12 = if h % 12 == 0 { 12 } else { h % 12 };
        let tokens = [
            ("HH", format!("{h:02}")),
            ("H", h.to_string()),
            ("hh", format!("{h12:02}")),
            ("h", h12.to_string()),
        ];
        let more = [
            ("mm", format!("{m:02}")),
            ("m", m.to_string()),
            ("ss", format!("{sec:02}")),
            ("s", sec.to_string()),
        ];
        let mut out = String::new();
        let mut rest = pattern;
        'outer: while !rest.is_empty() {
            for (token, text) in
                tokens.iter().chain(&more).chain(&[("a", if h < 12 { "AM" } else { "PM" }.into())])
            {
                if let Some(tail) = rest.strip_prefix(token) {
                    (out, rest) = (out + text, tail);
                    continue 'outer;
                }
            }
            let c = rest.chars().next().unwrap_or_default();
            out.push(c);
            rest = &rest[c.len_utf8()..];
        }
        out
    }

    fn keys_time(&self, b: &Binding<impl Sized>, end: f32) -> f32 {
        if let Some(id) = &b.timeline {
            let Some((tl, head)) = self.timelines.iter().find(|(tl, _)| &tl.id == id) else { return 0.0 };
            if head.playing {
                self.note(if head.t < 0.0 {
                    Wake::At(f64::from(-head.t / tl.speed.abs().max(1e-3)))
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
    pub(crate) fn get(&self, cx: &Context) -> T {
        let b = match self {
            Value::Fixed(v) => return v.clone(),
            Value::Bound(b) => b,
        };
        let raw = if let Some(keys) = &b.keys {
            sample(keys, cx.keys_time(b, keys.last().map_or(0.0, |k| k.t)))
        } else if let Some(id) = &b.prop {
            let prop = cx.props.get(id).unwrap_or(&Json::Null);
            let mapped = b.map.as_ref().and_then(|map| map.get(&key(prop)).cloned());
            mapped.or_else(|| T::from_json(prop)).unwrap_or_default()
        } else if let Some(pattern) = &b.clock {
            T::from_text(cx.clock(pattern)).unwrap_or_default()
        } else if let Some(path) = &b.script {
            cx.script.values.get(path).and_then(T::from_json).unwrap_or_default()
        } else {
            cx.signal(b).and_then(T::from_signal).unwrap_or_default()
        };
        let (mut comps, n) = raw.comps();
        comps[..n].iter_mut().for_each(|c| *c = b.modify(*c));
        if let Some(tau) = b.smooth.filter(|tau| *tau > 0.0) {
            let mut memo = cx.memo.borrow_mut();
            let last = memo.entry(&**b as *const Binding<T> as usize).or_insert(comps);
            let k = 1.0 - (-cx.dt / tau).exp();
            last[..n].iter_mut().zip(&comps).for_each(|(l, c)| *l += (c - *l) * k);
            if last[..n].iter().zip(&comps).any(|(l, c)| (l - c).abs() > 1e-3) {
                cx.note(Wake::Frame);
            }
            comps = *last;
        }
        if n == 0 { raw } else { raw.with_comps(comps) }
    }
}

pub(crate) fn key(value: &Json) -> String {
    match value {
        Json::String(s) => s.clone(),
        other => other.to_string(),
    }
}
