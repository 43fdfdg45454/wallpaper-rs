use super::{RETRIES, Stage};
use crate::power::Power;
use serde_json::json;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::{Mutex, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};
use wprs_core::QualityChoice;
use wprs_scene::{Budget, Quality, Step, Tier, Why};

pub(crate) fn retry() {
    wprs_render::retry_effects();
    if let Err(e) = wprs_scene::forgive_strikes() {
        log::warn!("cannot forget the effects that stopped the GPU or the app: {e}");
    }
    match crate::library::with(|lib| Ok(lib.recheck(wprs_scene::STOPPED)?)) {
        Ok(uids) if uids.is_empty() => {}
        Ok(_) => crate::store::changed(),
        Err(e) => log::warn!("cannot check the left-out packages again: {e}"),
    }
    RETRIES.fetch_add(1, Relaxed);
}

pub(crate) fn finish() {
    if !wprs_render::stop_builds() {
        log::warn!("a package shader is still building while the wallpaper closes");
    }
    wprs_scene::end_trials();
}

pub(crate) fn flashes() -> &'static str {
    ["", "slowed", "paused"][usize::from(shown().map_or(0, |s| s.marks[1]).min(2))]
}

pub(crate) fn particles() -> &'static str {
    ["", "gpu", "cpu"][usize::from(shown().map_or(0, |s| s.marks[0]).min(2))]
}

pub(crate) fn mark(slot: usize, which: usize, value: u8) {
    slots(|(_, shown)| shown[slot].marks[which] = value);
}

pub(crate) fn wallpaper() -> serde_json::Value {
    let shown = shown().unwrap_or(NOTHING);
    let (auto, over) = shown.cost.map_or(("", None), |kept| judged(kept, crate::power::choice()));
    let [tier, quality, reason] = level(shown.level).unwrap_or_default();
    let left: Vec<_> = shown.left.1.iter().map(|(id, why)| json!({ "id": id, "reason": why })).collect();
    let (frame, period, step) =
        shown.cost.map_or((None, None, ""), |c| (c.frame, Some(c.period), c.step.name()));
    json!({ "tier": tier, "quality": quality, "reason": reason, "left_out": left, "over": over, "auto": auto,
        "frame_ms": frame, "period_ms": period, "step": step })
}

fn level(level: u32) -> Option<[String; 3]> {
    let level = level.checked_sub(1)?;
    let why = Why::ALL.get(level as usize % 8)?.name();
    Some([format!("T{}", level / 32), format!("Q{}", level / 8 % 4), why.into()])
}

#[derive(Clone, Copy)]
pub(crate) struct Cost {
    pub(crate) budget: Budget,
    pub(crate) auto: Quality,
    pub(crate) quality: Quality,
    pub(crate) costs: [f32; 4],
    pub(crate) crowded: bool,
    pub(crate) stalled: bool,
    pub(crate) frame: Option<f32>,
    pub(crate) period: f32,
    pub(crate) step: Step,
}

pub(crate) const HOME: usize = 0;

#[derive(Clone)]
struct Shown {
    cost: Option<Cost>,
    level: u32,
    left: (u64, Vec<(String, String)>),
    marks: [u8; 2],
}

const NOTHING: Shown = Shown { cost: None, level: 0, left: (0, Vec::new()), marks: [0; 2] };

static SHOWN: Mutex<(usize, [Shown; 2])> = Mutex::new((0, [NOTHING, NOTHING]));

fn slots<T>(with: impl FnOnce(&mut (usize, [Shown; 2])) -> T) -> T {
    with(&mut SHOWN.lock().unwrap_or_else(PoisonError::into_inner))
}

pub(crate) fn keep(slot: usize, cost: Cost, level: u32) {
    slots(|(last, shown)| {
        *last = slot;
        (shown[slot].cost, shown[slot].level) = (Some(cost), level);
    });
}

pub(crate) fn leave(slot: usize, key: u64, left: impl FnOnce() -> Vec<(String, String)>) {
    slots(|(_, shown)| {
        if shown[slot].left.0 != key {
            shown[slot].left = (key, left());
        }
    });
}

pub(crate) fn forget(slot: usize) {
    slots(|(_, shown)| shown[slot] = NOTHING);
}

fn shown() -> Option<Shown> {
    slots(|(last, shown)| {
        [*last, 1 - *last].into_iter().map(|s| &shown[s]).find(|s| s.cost.is_some()).cloned()
    })
}

fn judged(kept: Cost, choice: QualityChoice) -> (&'static str, Option<[f32; 2]>) {
    let budget = Budget { choice, ..kept.budget };
    let quality = if budget == kept.budget { kept.quality } else { budget.choose((kept.auto, Why::User)).0 };
    let (cost, most) = (kept.costs[quality as usize], budget.tier.budget());
    let over = (!budget.choice.is_auto() && cost > most).then_some([cost, most]);
    let automatic = match () {
        _ if (kept.crowded || kept.stalled) && budget.choice.is_auto() => "still",
        _ if kept.crowded => "stop",
        _ if over.is_some() => "lower",
        _ => "",
    };
    (automatic, over)
}

impl Stage {
    pub(super) fn steer(&mut self, budget: Budget, power: Power, tier: Tier) -> ((Quality, Why), Quality) {
        let cooled = |(q, why): (Quality, Why)| {
            if power.ceiling() < q { (power.ceiling(), Why::Heat) } else { (q, why) }
        };
        let auto = cooled(budget.auto(&self.runtime));
        let ceiling = budget.choose(cooled(budget.reach()));
        self.pilot.start(budget.floor(), budget.choose(auto).0);
        let (quality, why) = self.pilot.level(ceiling, auto.1);
        if let Some(slot) = self.reports {
            let costs = [Quality::Q0, Quality::Q1, Quality::Q2, Quality::Q3].map(|q| self.runtime.cost(q));
            let (crowded, stalled) = (budget.crowded(&self.runtime), self.pilot.stalled());
            let period = 1000.0 / budget.timed_fps(&self.runtime, 0).max(1) as f32;
            let (frame, step) = (self.pilot.frame_ms(), self.pilot.step());
            let cost = Cost { budget, auto: auto.0, quality, costs, crowded, stalled, frame, period, step };
            keep(slot, cost, 1 + (tier as u32 * 4 + quality as u32) * 8 + why as u32);
        }
        ((quality, why), ceiling.0)
    }
}

pub(crate) fn wall() -> f64 {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let seconds = libc::time_t::try_from(now.as_secs()).unwrap_or_default();
    let mut local = unsafe { std::mem::zeroed::<libc::tm>() };
    let found = !unsafe { libc::localtime_r(&seconds, &mut local) }.is_null();
    now.as_secs_f64() + if found { local.tm_gmtoff as f64 } else { 0.0 }
}
