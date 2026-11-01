use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering::Relaxed};
use wprs_scene::{Quality, Tier};

const CHARGING_FPS: u32 = 60;
pub(crate) const BATTERY_FPS: u32 = 30;
const MAX_FPS: u32 = 60;
const PREVIEW_FPS: u32 = 30;
const HOT_FPS: u32 = 10;
const LIGHT: i32 = 1;
const MODERATE: i32 = 2;
const SEVERE: i32 = 3;
const CRITICAL: i32 = 4;
const WARM: f32 = 0.75;
const HIGH_CLASS: i32 = 33;

static CAPS: [AtomicU32; 2] = [AtomicU32::new(BATTERY_FPS), AtomicU32::new(CHARGING_FPS)];
static CLASS: AtomicI32 = AtomicI32::new(0);
static GUARD: AtomicBool = AtomicBool::new(false);
static HEARS: AtomicBool = AtomicBool::new(true);
static COMPUTES: AtomicBool = AtomicBool::new(true);

pub(crate) fn guard(on: bool, hears: bool) {
    GUARD.store(on, Relaxed);
    HEARS.store(hears, Relaxed);
}

pub(crate) fn hears() -> bool {
    HEARS.load(Relaxed)
}

pub(crate) fn guarded() -> bool {
    GUARD.load(Relaxed)
}

static POWER: std::sync::Mutex<Power> = std::sync::Mutex::new(Power::BATTERY);

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Power {
    pub(crate) charging: bool,
    pub(crate) saver: bool,
    pub(crate) low: bool,
    pub(crate) thermal: i32,
    pub(crate) headroom: f32,
    pub(crate) still: bool,
}

impl Power {
    pub(crate) fn now() -> Power {
        *POWER.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub(crate) fn set(self) {
        *POWER.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = self;
    }
}

impl Power {
    const BATTERY: Power =
        Power { charging: false, saver: false, low: false, thermal: 0, headroom: 0.0, still: false };

    fn paused(self) -> bool {
        self.saver || (self.low && !self.charging) || self.thermal >= CRITICAL
    }

    pub(crate) fn fps(self, preview: bool) -> u32 {
        self.capped(preview, CAPS.each_ref().map(|cap| cap.load(Relaxed)))
    }

    fn capped(self, preview: bool, caps: [u32; 2]) -> u32 {
        let full = caps[usize::from(self.charging)];
        let fps = match self.thermal {
            _ if self.paused() => 0,
            SEVERE.. => HOT_FPS.min(full),
            MODERATE.. => (full / 2).max(1),
            _ if self.headroom >= WARM => (full / 2).max(1),
            _ => full,
        };
        if preview { fps.min(PREVIEW_FPS) } else { fps }
    }

    pub(crate) fn moving(self) -> bool {
        !self.paused() && self.thermal < SEVERE
    }

    pub(crate) fn stills(self) -> Option<&'static str> {
        match () {
            _ if self.saver => Some("saver"),
            _ if self.low && !self.charging => Some("battery"),
            _ => (!self.moving()).then_some("heat"),
        }
    }

    pub(crate) fn ceiling(self) -> Quality {
        let level = match self.thermal {
            SEVERE.. => Quality::Q0,
            MODERATE => Quality::Q1,
            LIGHT => Quality::Q2,
            _ => Quality::Q3,
        };
        if self.headroom >= WARM { level.lower() } else { level }
    }
}

pub(crate) fn device(low_ram: bool, class: i32) {
    CLASS.store(if low_ram { -1 } else { class }, Relaxed);
}

pub(crate) fn computes() -> bool {
    COMPUTES.load(Relaxed)
}

pub(crate) fn tier(software: bool, compute: bool) -> Tier {
    COMPUTES.store(compute && !software, Relaxed);
    match CLASS.load(Relaxed) {
        _ if software || !compute => Tier::T0,
        class if class < 0 => Tier::T0,
        HIGH_CLASS.. => Tier::T2,
        _ => Tier::T1,
    }
}

pub(crate) fn caps(battery: u32, charging: u32) {
    CAPS[0].store(battery.clamp(1, MAX_FPS), Relaxed);
    CAPS[1].store(charging.clamp(1, MAX_FPS), Relaxed);
}

pub(crate) fn delay_ms(fps: u32) -> i64 {
    let interval = 1000.0 / f64::from(fps.max(1));
    (interval - (interval / 4.0).min(8.0)).floor().max(1.0) as i64
}

#[cfg(test)]
mod tests;
