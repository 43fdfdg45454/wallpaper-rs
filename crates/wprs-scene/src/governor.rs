use crate::{Budget, Heat, Quality, Runtime, Why};
use std::time::{Duration, Instant};

const LOWER: Duration = Duration::from_secs(2);
const RAISE: Duration = Duration::from_secs(15);
const APART: Duration = Duration::from_secs(5);
const BREAK: Duration = Duration::from_secs(1);
const WEIGHT: f32 = 0.2;
const MOST: f32 = 16.0;
const MARGIN: f32 = 1.5;
const PROBE: Duration = Duration::from_secs(60);
const BOUNCE: Duration = Duration::from_secs(30);
const TRIES: u8 = 4;
const AHEAD: f32 = 1.15;
pub const PHONE_SHARE: f32 = 0.75;

impl Budget {
    pub fn auto(&self, runtime: &Runtime) -> (Quality, Why) {
        let steps = |from: Quality, to: Quality, why: Why, now: Why| if to < from { why } else { now };
        let base = self.tier.quality(false);
        let why = if base < Quality::Q3 { Why::Device } else { Why::Full };
        let strained = self.tier.quality(self.on_battery || self.saver);
        let why = steps(base, strained, if self.on_battery { Why::Battery } else { Why::Saver }, why);
        let start = if self.saver { strained.lower() } else { strained };
        let why = steps(strained, start, Why::Saver, why);
        let fitted = Quality::fitting(|q| runtime.cost(q), self.tier, start);
        let why = steps(start, fitted, Why::Cost, why);
        let heated = match self.heat {
            Heat::Cool => fitted,
            Heat::Warm => fitted.lower(),
            Heat::Hot => Quality::Q0,
        };
        (heated, steps(fitted, heated, Why::Heat, why))
    }

    pub fn reach(&self) -> (Quality, Why) {
        let (start, why) = match self.saver {
            true => (self.tier.quality(true).lower(), Why::Saver),
            false => (Quality::Q3, Why::Full),
        };
        match self.heat {
            Heat::Cool => (start, why),
            Heat::Warm => (start.lower(), Why::Heat),
            Heat::Hot => (Quality::Q0, Why::Heat),
        }
    }

    pub fn steady(&self) -> Quality {
        let strained = self.tier.quality(self.on_battery || self.saver);
        let start = if self.saver { strained.lower() } else { strained };
        let auto = match self.heat {
            Heat::Cool => start,
            Heat::Warm => start.lower(),
            Heat::Hot => Quality::Q0,
        };
        self.choose((auto, Why::Full)).0
    }

    pub fn gpu_share(&self) -> f32 {
        if self.on_battery || self.saver { 0.25 } else { 0.5 }
    }

    pub fn raises(&self) -> bool {
        !self.on_battery && !self.saver && self.heat == Heat::Cool
    }
}

#[derive(Debug, Clone, Default)]
pub struct Governor {
    ewma: f32,
    last: Option<Instant>,
    over: Option<Instant>,
    under: Option<Instant>,
    held: Option<Instant>,
    changed: Option<Instant>,
    raised: Option<Instant>,
    steps: u8,
    tries: u8,
    probes: u8,
    probing: bool,
    gains: [f32; 4],
    gauge: Option<f32>,
    floor: Option<Quality>,
    timed: bool,
}

impl Governor {
    pub fn timed() -> Governor {
        Governor { timed: true, ..Governor::default() }
    }

    pub fn hold(&mut self, floor: Quality) {
        if self.floor.unwrap_or(Quality::Q0) != floor {
            *self = Governor { floor: Some(floor), timed: self.timed, ..Governor::default() };
        }
    }

    pub fn begin(&mut self, ceiling: Quality, fitted: Quality) {
        if self.last.is_none() && self.changed.is_none() {
            let mut q = ceiling;
            self.steps = 0;
            while q > fitted.min(ceiling) {
                (q, self.steps) = (q.lower(), self.steps + 1);
            }
        }
    }

    pub fn level(&self, (ceiling, why): (Quality, Why)) -> (Quality, Why) {
        let floor = self.floor.unwrap_or(Quality::Q0).min(ceiling);
        let lowered = (0..self.steps).fold(ceiling, |q, _| q.lower()).max(floor);
        if lowered < ceiling { (lowered, Why::Slow) } else { (ceiling, why) }
    }

    pub fn feed(
        &mut self,
        at: Instant,
        gpu_ms: f32,
        budget: impl Fn(Quality) -> (f32, f32),
        ceiling: Quality,
        raises: bool,
    ) -> bool {
        let broken = self.last.is_none_or(|last| at.saturating_duration_since(last) > BREAK);
        self.ewma = if broken { gpu_ms } else { self.ewma + WEIGHT * (gpu_ms - self.ewma) };
        if broken {
            (self.over, self.under, self.held) = (None, None, None);
        }
        self.last = Some(at);
        if self.raised.is_some_and(|r| at.saturating_duration_since(r) >= BOUNCE) {
            match std::mem::take(&mut self.probing) {
                true => self.probes = 0,
                false => self.tries = self.tries.saturating_sub(1),
            }
            self.raised = None;
        }
        let settled = self.changed.is_none_or(|c| at.saturating_duration_since(c) >= APART);
        let (steps, index) = (usize::from(self.steps), usize::from(self.steps).wrapping_sub(1));
        if settled
            && let Some(peak) = self.gauge.take()
            && let Some(g) = self.gains.get_mut(index)
        {
            *g = (peak / self.ewma.max(f32::EPSILON)).clamp(1.0, MOST);
        }
        let here = self.level((ceiling, Why::Full)).0;
        let above = (1..steps).fold(ceiling, |q, _| q.lower());
        let ((limit, cost), (high, dearer)) = (budget(here), budget(above));
        let expected = (dearer / cost.max(f32::EPSILON) * MARGIN).clamp(1.0, MOST);
        let measured = self.gains.get(index).filter(|g| **g > 0.0).map(|g| g.clamp(1.0, expected));
        let slow = self.ewma > limit && here > self.floor.unwrap_or(Quality::Q0);
        let ratio = measured.unwrap_or((expected / MARGIN).max(1.0)) * AHEAD;
        let fits =
            steps > 0 && raises && if self.timed { self.ewma * ratio < high } else { self.ewma < high / 2.0 };
        let fast = fits && (self.timed || self.ewma * measured.unwrap_or(1.0) < high / 2.0);
        let room = self.timed && steps > 0 && raises && !fits && self.ewma * AHEAD < high;
        let since = |mark: &mut Option<Instant>, on: bool| {
            *mark = if on { Some(mark.unwrap_or(at)) } else { None };
            mark.map(|m| at.saturating_duration_since(m))
        };
        let (over, under) = (since(&mut self.over, slow), since(&mut self.under, fast));
        let held = since(&mut self.held, fits && !fast || room);
        let waited = |wait: Duration, d: Option<Duration>| d.is_some_and(|d| d >= wait * (1 << self.tries));
        let probed = held.is_some_and(|d| d >= RAISE * (1 << self.probes));
        let (step, probe) = match () {
            _ if settled && over.is_some_and(|d| d >= LOWER) => (1, false),
            _ if settled && (waited(RAISE, under) || !self.timed && waited(PROBE, held)) => (-1, false),
            _ if settled && self.timed && probed => (-1, true),
            _ => return false,
        };
        if step > 0 && self.raised.take().is_some() {
            match std::mem::take(&mut self.probing) {
                true => self.probes = (self.probes + 1).min(TRIES),
                false => self.tries = (self.tries + 1).min(TRIES),
            }
        }
        self.probing = probe;
        (self.raised, self.gauge) = ((step < 0).then_some(at), (step > 0).then_some(self.ewma));
        self.steps = self.steps.saturating_add_signed(step);
        (self.changed, self.over, self.under, self.held) = (Some(at), None, None, None);
        true
    }
}
