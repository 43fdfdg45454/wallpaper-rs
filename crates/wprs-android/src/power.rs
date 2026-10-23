use std::sync::atomic::{AtomicI32, AtomicU32, Ordering::Relaxed};
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

static POWER: std::sync::Mutex<Power> = std::sync::Mutex::new(Power::BATTERY);

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Power {
    pub(crate) charging: bool,
    pub(crate) saver: bool,
    pub(crate) low: bool,
    pub(crate) thermal: i32,
    pub(crate) headroom: f32,
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
    const BATTERY: Power = Power { charging: false, saver: false, low: false, thermal: 0, headroom: 0.0 };

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

pub(crate) fn tier(software: bool, compute: bool) -> Tier {
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
mod tests {
    use super::*;

    fn on(f: impl FnOnce(&mut Power)) -> Power {
        let mut power = Power::BATTERY;
        f(&mut power);
        power
    }

    #[test]
    fn the_charger_doubles_the_cap_and_previews_stay_at_thirty() {
        let charging = on(|p| p.charging = true);
        assert_eq!((Power::BATTERY.fps(false), charging.fps(false), charging.fps(true)), (30, 60, 30));
    }

    #[test]
    fn saving_power_or_a_low_battery_pauses_everything() {
        for power in [on(|p| p.saver = true), on(|p| p.low = true), on(|p| p.thermal = CRITICAL)] {
            assert_eq!((power.fps(false), power.moving()), (0, false));
        }
        assert_eq!(on(|p| (p.low, p.charging) = (true, true)).fps(false), 60);
    }

    #[test]
    fn a_normal_battery_keeps_moving_and_each_pause_says_why() {
        assert_eq!((Power::BATTERY.moving(), Power::BATTERY.stills()), (true, None));
        let warm = on(|p| (p.thermal, p.headroom) = (MODERATE, 0.9));
        assert_eq!((warm.moving(), warm.stills(), warm.fps(false)), (true, None, 15));
        let reasons =
            [on(|p| p.saver = true), on(|p| p.low = true), on(|p| p.thermal = SEVERE)].map(Power::stills);
        assert_eq!(reasons, [Some("saver"), Some("battery"), Some("heat")]);
        assert_eq!(on(|p| (p.low, p.charging) = (true, true)).stills(), None);
    }

    #[test]
    fn heat_lowers_the_rate_before_pausing() {
        assert_eq!(on(|p| p.headroom = 0.8).fps(false), 15);
        assert_eq!(on(|p| (p.thermal, p.charging) = (MODERATE, true)).fps(false), 30);
        let severe = on(|p| p.thermal = SEVERE);
        assert_eq!((severe.fps(false), severe.moving()), (10, false));
    }

    #[test]
    fn the_chosen_caps_apply_before_heat() {
        let charging = on(|p| p.charging = true);
        assert_eq!((Power::BATTERY.capped(false, [15, 30]), charging.capped(true, [10, 60])), (15, 30));
        assert_eq!(on(|p| p.headroom = 0.8).capped(false, [10, 30]), 5);
    }

    #[test]
    fn heat_lowers_the_scene_level_and_the_device_class_picks_the_tier() {
        let levels = [0, LIGHT, MODERATE, SEVERE].map(|t| on(|p| p.thermal = t).ceiling());
        assert_eq!(levels, [Quality::Q3, Quality::Q2, Quality::Q1, Quality::Q0]);
        assert_eq!(on(|p| (p.thermal, p.headroom) = (LIGHT, 0.9)).ceiling(), Quality::Q1);
        let tiers = [(false, 0), (false, 34), (true, 34)].map(|(low, class)| {
            device(low, class);
            tier(false, true)
        });
        assert_eq!(tiers, [Tier::T1, Tier::T2, Tier::T0]);
        device(false, 34);
        assert_eq!((tier(true, true), tier(false, false)), (Tier::T0, Tier::T0));
        device(false, 0);
    }

    #[test]
    fn the_wait_leaves_room_for_the_next_vsync() {
        assert_eq!([60, 30, 10, 0].map(delay_ms), [12, 25, 92, 992]);
    }
}
