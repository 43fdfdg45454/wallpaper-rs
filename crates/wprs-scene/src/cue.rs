use crate::animate::Animate;
use crate::resolve::Context;
use crate::timeline::{Key, Loop, Playhead, sample};
use crate::value::Binding;
use crate::{Runtime, Timeline, Wake};
use serde::{Deserialize, Serialize};

pub const MAX_FIRED: usize = 64;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fired {
    pub timeline: String,
    pub name: Option<String>,
    pub t: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Course {
    pub id: String,
    pub time: f32,
    pub duration: f32,
    pub playing: bool,
    pub speed: f32,
}

impl Timeline {
    pub(crate) fn crossed(&self, from: f32, to: f32) -> Vec<(f32, &str)> {
        let (d, forward) = (self.length(), to >= from);
        let (lo, hi) = if forward { (from, to) } else { (to, from) };
        let mut out = Vec::new();
        if d <= 0.0 || (forward && hi < 0.0) || lo == hi {
            return out;
        }
        let inside = |x: f32| if forward { (lo < x || lo <= 0.0) && x <= hi } else { lo <= x && x < hi };
        for e in &self.events {
            let (period, mirror) = match self.repeat {
                Loop::Once => (f32::INFINITY, None),
                Loop::Repeat => (d, None),
                Loop::Pingpong => (2.0 * d, (e.t > 0.0 && e.t < d).then_some(2.0 * d - e.t)),
            };
            for first in [Some(e.t), mirror].into_iter().flatten() {
                let floor = if forward { 0.0 } else { f32::NEG_INFINITY };
                let k = if period.is_finite() { ((lo - first) / period).floor().max(floor) } else { 0.0 };
                let mut x = if k != 0.0 { first + k * period } else { first };
                while x <= hi && out.len() < MAX_FIRED {
                    if inside(x) {
                        out.push((x, e.name.as_str()));
                    }
                    x += period;
                }
            }
        }
        out.sort_by(|a, b| if forward { a.0.total_cmp(&b.0) } else { b.0.total_cmp(&a.0) });
        out
    }

    pub(crate) fn next_event(&self, head: &Playhead, heard: impl Fn(&str) -> bool) -> Option<f64> {
        if !head.playing || self.events.is_empty() || head.rate == 0.0 {
            return None;
        }
        let ahead = if head.reversed == (head.rate < 0.0) { 1.0 } else { -1.0 };
        let reach = self.length().max(1e-3) * 2.0;
        let crossed = self.crossed(head.t, head.t + ahead * reach);
        let next = crossed.iter().find(|(_, name)| heard(name)).map(|(x, _)| (x - head.t).abs());
        next.map(|gap| f64::from(gap / head.rate.abs()))
    }

    pub(crate) fn wraps(&self) -> Option<f32> {
        (self.wrap && self.repeat == Loop::Repeat).then(|| self.length())
    }
}

pub(crate) fn wrapped<T: Animate>(keys: &[Key<T>], t: f32, wrap: Option<f32>) -> T {
    let (Some(d), Some(first), Some(last)) = (wrap, keys.first(), keys.last()) else {
        return sample(keys, t);
    };
    let span = first.t + d - last.t;
    let gone = match t {
        t if t > last.t => t - last.t,
        t if t < first.t => t + d - last.t,
        _ => return sample(keys, t),
    };
    match span > 1e-6 {
        true => last.v.lerp(&first.v, last.ease.apply((gone / span).clamp(0.0, 1.0))),
        false => sample(keys, t),
    }
}

fn heard(tl: &Timeline, head: &Playhead, from: f32, was: bool, out: &mut Vec<(Fired, f32)>) {
    let late = |x: f32| (head.t - x).abs() / head.rate.abs().max(1e-3);
    for (x, name) in tl.crossed(from, head.t) {
        let t = tl.events.iter().find(|e| e.name == name).map_or(0.0, |e| e.t);
        out.push((Fired { timeline: tl.id.clone(), name: Some(name.to_owned()), t }, late(x)));
    }
    if was && !head.playing {
        out.push((Fired { timeline: tl.id.clone(), name: None, t: tl.position(head) }, 0.0));
    }
}

impl Runtime {
    pub(crate) fn tick(&mut self, step: f32) {
        let mut names = Vec::new();
        for (tl, head) in &mut self.timelines {
            let (from, was) = (head.t, head.playing);
            tl.advance(head, step);
            heard(tl, head, from, was, &mut names);
        }
        self.spread(names);
    }

    pub(crate) fn spread(&mut self, mut names: Vec<(Fired, f32)>) {
        let mut i = 0;
        while let Some((fired, late)) = names.get(i) {
            let (wanted, late) = (fired.name.as_ref().map(|n| format!("event:{n}")), *late);
            i += 1;
            let Some(wanted) = wanted else { continue };
            for at in self.trigger_late(|t| t == wanted, late) {
                let (tl, head) = &self.timelines[at];
                if names.len() < 4 * MAX_FIRED {
                    heard(tl, head, tl.origin(head.rate), false, &mut names);
                }
            }
        }
        let room = MAX_FIRED.saturating_sub(self.fired.len());
        self.fired.extend(names.into_iter().map(|(f, _)| f).take(room));
    }

    pub fn take_fired(&mut self) -> Vec<Fired> {
        std::mem::take(&mut self.fired)
    }

    pub fn playbacks(&self) -> Vec<Course> {
        let mut out = Vec::new();
        self.fill_playbacks(&mut out);
        out
    }

    pub fn fill_playbacks(&self, out: &mut Vec<Course>) {
        out.truncate(self.timelines.len());
        for (i, (tl, head)) in self.timelines.iter().enumerate() {
            let (time, duration, playing, speed) = (tl.position(head), tl.length(), head.playing, head.rate);
            match out.get_mut(i) {
                Some(c) if c.id == tl.id => {
                    (c.time, c.duration, c.playing, c.speed) = (time, duration, playing, speed)
                }
                _ => {
                    out.truncate(i);
                    out.push(Course { id: tl.id.clone(), time, duration, playing, speed });
                }
            }
        }
    }

    pub(crate) fn cue_wake(&self) -> Wake {
        let heard = |name: &str| self.listened.contains(name);
        let due = self.timelines.iter().filter(|(tl, _)| tl.events.iter().any(|e| heard(&e.name)));
        due.filter_map(|(tl, head)| tl.next_event(head, heard)).map(Wake::At).fold(Wake::Idle, Wake::min)
    }
}

impl Context<'_> {
    pub(crate) fn wrap<T>(&self, b: &Binding<T>) -> Option<f32> {
        let id = b.timeline.as_ref()?;
        self.timelines.iter().find(|(tl, _)| &tl.id == id)?.0.wraps()
    }

    pub(crate) fn rate<T>(&self, b: &Binding<T>) -> f32 {
        let found = b.timeline.as_ref().and_then(|id| self.timelines.iter().find(|(tl, _)| &tl.id == id));
        found.map_or(1.0, |(_, head)| head.rate.abs().max(1e-3))
    }
}
