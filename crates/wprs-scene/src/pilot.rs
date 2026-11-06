use crate::{Quality, Why};
use serde::Serialize;
use std::time::{Duration, Instant};

const SPAN: usize = 32;
const LEAST: usize = 12;
const DROP: f32 = 0.85;
const CLIMB: f32 = 0.5;
const FIT: f32 = 0.75;
const STALL: f32 = 2.0;
const LOWER: Duration = Duration::from_secs(2);
const RAISE: Duration = Duration::from_secs(6);
const APART: Duration = Duration::from_secs(5);
const BOUNCE: Duration = Duration::from_secs(30);
const BREAK: Duration = Duration::from_secs(1);
const TRIES: u8 = 4;

#[derive(Debug, Clone, Copy, Default)]
pub struct Window {
    times: [f32; SPAN],
    len: usize,
    next: usize,
}

impl Window {
    pub fn push(&mut self, ms: f32) {
        self.times[self.next] = ms;
        (self.next, self.len) = ((self.next + 1) % SPAN, (self.len + 1).min(SPAN));
    }

    pub fn mean(&self) -> Option<f32> {
        (self.len >= LEAST).then_some(())?;
        let mut sorted = self.times;
        let sorted = &mut sorted[..self.len];
        sorted.sort_unstable_by(f32::total_cmp);
        let median = sorted[self.len / 2];
        let kept = sorted.iter().filter(|t| **t <= median * 2.0 && **t >= median / 2.0);
        let (sum, n) = kept.fold((0.0, 0u32), |(s, n), t| (s + t, n + 1));
        Some(if n == 0 { median } else { sum / n as f32 })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    #[default]
    Model,
    Steady,
    Climbed,
    Dropped,
    Top,
    Held,
    Still,
}

impl Step {
    pub fn name(self) -> &'static str {
        ["model", "steady", "climbed", "dropped", "top", "held", "still"][self as usize]
    }
}

#[derive(Debug, Clone, Default)]
pub struct Pilot {
    window: Window,
    start: Option<(Quality, Quality)>,
    level: Quality,
    last: Option<Instant>,
    over: Option<Instant>,
    under: Option<Instant>,
    changed: Option<Instant>,
    climbed: Option<(Instant, f32)>,
    gains: [f32; 4],
    tries: u8,
    step: Step,
    frame: Option<f32>,
    stalled: bool,
}

impl Pilot {
    pub fn start(&mut self, floor: Quality, fitted: Quality) {
        let key = Some((floor, fitted.max(floor)));
        if self.start != key {
            let stalled = self.stalled && self.start.is_some_and(|s| s.0 == floor);
            *self = Pilot { start: key, level: fitted.max(floor), stalled, ..Pilot::default() };
        }
    }

    fn floor(&self, ceiling: Quality) -> Quality {
        self.start.map_or(Quality::Q0, |s| s.0).min(ceiling)
    }

    pub fn level(&self, (ceiling, why): (Quality, Why), model: Why) -> (Quality, Why) {
        let here = self.level.clamp(self.floor(ceiling), ceiling);
        match () {
            _ if here == ceiling => (ceiling, why),
            _ if self.step == Step::Model => (here, model),
            _ => (here, Why::Slow),
        }
    }

    pub fn stalled(&self) -> bool {
        self.stalled
    }

    pub fn frame_ms(&self) -> Option<f32> {
        self.frame
    }

    pub fn step(&self) -> Step {
        self.step
    }

    pub fn feed(
        &mut self,
        at: Instant,
        ms: f32,
        period: f32,
        (ceiling, raises, stills): (Quality, bool, bool),
    ) -> bool {
        if self.last.is_none_or(|l| at.saturating_duration_since(l) > BREAK) {
            self.over = None;
        }
        self.last = Some(at);
        if ms.is_finite() && ms >= 0.0 {
            self.window.push(ms);
        }
        let Some(mean) = self.window.mean() else { return false };
        self.frame = Some(mean);
        let floor = self.floor(ceiling);
        let here = self.level.clamp(floor, ceiling);
        self.level = here;
        let settled = self.changed.is_none_or(|c| at.saturating_duration_since(c) >= APART);
        if let Some((when, before)) = self.climbed.filter(|_| settled) {
            self.gains[here as usize] = (mean / before.max(f32::EPSILON)).max(1.0);
            if at.saturating_duration_since(when) >= BOUNCE {
                (self.climbed, self.tries) = (None, self.tries.saturating_sub(1));
            }
        }
        let above = Quality::from_level(here as u8 + 1);
        let gain = self.gains[above as usize];
        let fits = match gain {
            _ if here >= ceiling || !raises => false,
            g if g > 0.0 => mean * g < period * FIT,
            _ => mean < period * CLIMB,
        };
        let slow = if here > floor { mean > period * DROP } else { stills && mean > period * STALL };
        let over = lasted(&mut self.over, slow, at, LOWER);
        let under = lasted(&mut self.under, fits, at, RAISE * (1 << self.tries));
        if !settled {
            return false;
        }
        let (level, step) = match () {
            _ if over && here == floor => (here, Step::Still),
            _ if over => (here.lower(), Step::Dropped),
            _ if under => (above, Step::Climbed),
            _ if here >= ceiling || !raises => return self.hold(Step::Top),
            _ if gain > 0.0 && !fits => return self.hold(Step::Held),
            _ => return self.hold(Step::Steady),
        };
        if step != Step::Climbed && self.climbed.take().is_some() {
            self.tries = (self.tries + 1).min(TRIES);
        }
        if step == Step::Climbed {
            self.climbed = Some((at, mean));
        }
        (self.level, self.step, self.stalled) = (level, step, self.stalled || step == Step::Still);
        (self.window, self.changed, self.over, self.under) = (Window::default(), Some(at), None, None);
        true
    }

    fn hold(&mut self, step: Step) -> bool {
        self.step = step;
        false
    }
}

fn lasted(mark: &mut Option<Instant>, on: bool, at: Instant, wait: Duration) -> bool {
    *mark = if on { Some(mark.unwrap_or(at)) } else { None };
    mark.is_some_and(|m| at.saturating_duration_since(m) >= wait)
}
