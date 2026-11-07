use crate::{Quality, Why};
use serde::Serialize;
use std::time::{Duration, Instant};

mod pace;
mod window;

pub use pace::{HALVES, halved, periods};
pub use window::Window;

const DROP: f32 = 0.85;
const CLIMB: f32 = 0.5;
const FIT: f32 = 0.75;
const STALL: f32 = 2.0;
const LOWER: Duration = Duration::from_secs(2);
const RAISE: Duration = Duration::from_secs(6);
const APART: Duration = Duration::from_secs(5);
const BOUNCE: Duration = Duration::from_secs(30);
const FLIPS: Duration = Duration::from_secs(120);
const BREAK: Duration = Duration::from_secs(1);
const TRIES: u8 = 4;

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
    Halved,
    Doubled,
}

impl Step {
    pub fn name(self) -> &'static str {
        ["model", "steady", "climbed", "dropped", "top", "held", "still", "halved", "doubled"][self as usize]
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
    flips: u8,
    halved: bool,
    paced: Option<Instant>,
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

    pub fn pace(&self) -> u32 {
        1 + u32::from(self.halved)
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
        periods: [f32; 2],
        (ceiling, raises, stills): (Quality, bool, bool),
    ) -> bool {
        if self.last.is_none_or(|l| at.saturating_duration_since(l) > BREAK) {
            self.over = None;
        }
        self.last = Some(at);
        if self.paced.is_some_and(|p| at.saturating_duration_since(p) >= FLIPS) {
            self.flips = self.flips.saturating_sub(1);
            self.paced = (self.flips > 0).then_some(at);
        }
        if ms.is_finite() && ms >= 0.0 {
            self.window.push(ms);
        }
        let halves = periods[1] > periods[0] * HALVES;
        if self.halved && !halves {
            (self.halved, self.step) = (false, Step::Doubled);
            return self.moved(at);
        }
        let Some(mean) = self.window.mean() else { return false };
        self.frame = Some(mean);
        let (floor, period) = (self.floor(ceiling), periods[usize::from(self.halved)]);
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
        let room = here < ceiling && raises;
        let lift = |p: f32| room && if gain > 0.0 { mean * gain < p * FIT } else { mean < p * CLIMB };
        let fits = lift(period);
        let up = fits || (!self.halved && halves && lift(periods[1]));
        let lighter = self.halved && raises && mean < periods[0] * CLIMB;
        let slow = if here > floor { mean > period * DROP } else { stills && mean > period * STALL };
        let over = lasted(&mut self.over, slow, at, LOWER);
        let tries = if lighter { self.tries.max(self.flips) } else { self.tries };
        let under = lasted(&mut self.under, up || lighter, at, RAISE * (1 << tries));
        if !settled {
            return false;
        }
        let (level, halved, step) = match () {
            _ if over && here == floor => (here, self.halved, Step::Still),
            _ if over && !self.halved && halves => (here, true, Step::Halved),
            _ if over => (here.lower(), self.halved, Step::Dropped),
            _ if under && lighter => (here, false, Step::Doubled),
            _ if under && fits => (above, self.halved, Step::Climbed),
            _ if under => (above, true, Step::Climbed),
            _ if here >= ceiling || !raises => return self.hold(Step::Top),
            _ if gain > 0.0 && !up => return self.hold(Step::Held),
            _ => return self.hold(Step::Steady),
        };
        if !matches!(step, Step::Climbed | Step::Doubled) && self.climbed.take().is_some() {
            self.tries = (self.tries + 1).min(TRIES);
        }
        if step == Step::Climbed {
            self.climbed = Some((at, mean));
        }
        if halved != self.halved {
            let quick = self.paced.is_some_and(|p| at.saturating_duration_since(p) < FLIPS);
            (self.flips, self.paced) = (self.flips + u8::from(quick), Some(at));
            self.flips = self.flips.min(TRIES);
        }
        (self.level, self.halved, self.step) = (level, halved, step);
        self.stalled |= step == Step::Still;
        self.moved(at)
    }

    fn moved(&mut self, at: Instant) -> bool {
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
