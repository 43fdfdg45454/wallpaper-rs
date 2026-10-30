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

impl Budget {
    pub fn level(&self, runtime: &Runtime) -> (Quality, Why) {
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
    gains: [f32; 4],
    gauge: Option<f32>,
}

impl Governor {
    pub fn level(&self, (ceiling, why): (Quality, Why)) -> (Quality, Why) {
        let lowered = (0..self.steps).fold(ceiling, |q, _| q.lower());
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
            (self.raised, self.tries) = (None, self.tries.saturating_sub(1));
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
        let gain = self.gains.get(index).map_or(1.0, |g| g.clamp(1.0, expected));
        let slow = self.ewma > limit && here > Quality::Q0;
        let fits = steps > 0 && raises && self.ewma < high / 2.0;
        let fast = fits && self.ewma * gain < high / 2.0;
        let since = |mark: &mut Option<Instant>, on: bool| {
            *mark = if on { Some(mark.unwrap_or(at)) } else { None };
            mark.map(|m| at.saturating_duration_since(m))
        };
        let (over, under) = (since(&mut self.over, slow), since(&mut self.under, fast));
        let held = since(&mut self.held, fits && !fast);
        let waited = |wait: Duration, d: Option<Duration>| d.is_some_and(|d| d >= wait * (1 << self.tries));
        let step = match () {
            _ if settled && over.is_some_and(|d| d >= LOWER) => 1,
            _ if settled && (waited(RAISE, under) || waited(PROBE, held)) => -1,
            _ => return false,
        };
        if step > 0 && self.raised.take().is_some() {
            self.tries = (self.tries + 1).min(TRIES);
        }
        (self.raised, self.gauge) = ((step < 0).then_some(at), (step > 0).then_some(self.ewma));
        self.steps = self.steps.saturating_add_signed(step);
        (self.changed, self.over, self.under, self.held) = (Some(at), None, None, None);
        true
    }
}
