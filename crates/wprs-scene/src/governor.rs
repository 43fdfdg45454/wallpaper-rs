use crate::{Budget, Heat, Quality, Runtime, Why};
use std::time::{Duration, Instant};

const LOWER: Duration = Duration::from_secs(2);
const RAISE: Duration = Duration::from_secs(15);
const APART: Duration = Duration::from_secs(5);
const BREAK: Duration = Duration::from_secs(1);
const WEIGHT: f32 = 0.2;
const MOST: f32 = 16.0;

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
    changed: Option<Instant>,
    steps: u8,
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
        budget: impl Fn(Quality) -> f32,
        ceiling: Quality,
        raises: bool,
    ) -> bool {
        let broken = self.last.is_none_or(|last| at.saturating_duration_since(last) > BREAK);
        self.ewma = if broken { gpu_ms } else { self.ewma + WEIGHT * (gpu_ms - self.ewma) };
        if broken {
            (self.over, self.under) = (None, None);
        }
        self.last = Some(at);
        let settled = self.changed.is_none_or(|c| at.saturating_duration_since(c) >= APART);
        let steps = usize::from(self.steps);
        if settled && let Some(peak) = self.gauge.take() {
            let gain = (peak / self.ewma.max(f32::EPSILON)).clamp(1.0, MOST);
            if let Some(g) = self.gains.get_mut(steps.wrapping_sub(1)) {
                *g = gain;
            }
        }
        let here = self.level((ceiling, Why::Full)).0;
        let above = (1..steps).fold(ceiling, |q, _| q.lower());
        let gain = self.gains.get(steps.wrapping_sub(1)).map_or(1.0, |g| g.max(1.0));
        let slow = self.ewma > budget(here) && here > Quality::Q0;
        let fast = steps > 0 && raises && self.ewma * gain < budget(above) / 2.0;
        let since = |mark: &mut Option<Instant>, on: bool| {
            *mark = if on { Some(mark.unwrap_or(at)) } else { None };
            mark.map(|m| at.saturating_duration_since(m))
        };
        let (over, under) = (since(&mut self.over, slow), since(&mut self.under, fast));
        let step = match () {
            _ if settled && over.is_some_and(|d| d >= LOWER) => 1,
            _ if settled && under.is_some_and(|d| d >= RAISE) => -1,
            _ => return false,
        };
        self.gauge = (step > 0).then_some(self.ewma);
        self.steps = self.steps.saturating_add_signed(step);
        (self.changed, self.over, self.under) = (Some(at), None, None);
        true
    }
}
