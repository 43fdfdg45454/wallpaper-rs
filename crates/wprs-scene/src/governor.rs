use crate::{Budget, Heat, Quality, Runtime};
use serde::Serialize;
use std::time::{Duration, Instant};

const LOWER: Duration = Duration::from_secs(2);
const RAISE: Duration = Duration::from_secs(15);
const APART: Duration = Duration::from_secs(5);
const BREAK: Duration = Duration::from_secs(1);
const WEIGHT: f32 = 0.2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Why {
    #[default]
    Full,
    Device,
    Cost,
    Battery,
    Saver,
    Heat,
    Slow,
}

impl Why {
    pub const ALL: [Why; 7] =
        [Why::Full, Why::Device, Why::Cost, Why::Battery, Why::Saver, Why::Heat, Why::Slow];

    pub fn name(self) -> &'static str {
        ["full", "device", "cost", "battery", "saver", "heat", "slow"][self as usize]
    }
}

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
}

impl Governor {
    pub fn level(&self, (ceiling, why): (Quality, Why)) -> (Quality, Why) {
        let lowered = (0..self.steps).fold(ceiling, |q, _| q.lower());
        if lowered < ceiling { (lowered, Why::Slow) } else { (ceiling, why) }
    }

    pub fn feed(&mut self, at: Instant, gpu_ms: f32, budget_ms: f32, ceiling: Quality, raises: bool) -> bool {
        let broken = self.last.is_none_or(|last| at.saturating_duration_since(last) > BREAK);
        self.ewma = if broken { gpu_ms } else { self.ewma + WEIGHT * (gpu_ms - self.ewma) };
        if broken {
            (self.over, self.under) = (None, None);
        }
        self.last = Some(at);
        let settled = self.changed.is_none_or(|c| at.saturating_duration_since(c) >= APART);
        let lowest = self.level((ceiling, Why::Full)).0 == Quality::Q0;
        let (slow, fast) =
            (self.ewma > budget_ms && !lowest, self.ewma < budget_ms / 2.0 && self.steps > 0 && raises);
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
        self.steps = self.steps.saturating_add_signed(step);
        (self.changed, self.over, self.under) = (Some(at), None, None);
        true
    }
}
