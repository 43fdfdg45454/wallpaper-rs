use crate::{Quality, Runtime, Tier};
use wprs_core::settings::fps::DISPLAY;

const BATTERY: u32 = 30;
const UNKNOWN: u32 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Heat {
    #[default]
    Cool,
    Warm,
    Hot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    pub tier: Tier,
    pub on_battery: bool,
    pub max_fps: u32,
    pub saver: bool,
    pub heat: Heat,
}

fn display(refresh: u32) -> u32 {
    if refresh == 0 { UNKNOWN } else { refresh }
}

fn known(refresh: u32, rate: u32) -> u32 {
    if refresh == 0 { rate } else { rate.min(refresh) }
}

impl Heat {
    fn fps(self) -> u32 {
        [u32::MAX, 30, 15][self as usize]
    }
}

impl Budget {
    pub fn new(tier: Tier, on_battery: bool, max_fps: u32) -> Budget {
        Budget { tier, on_battery, max_fps, saver: false, heat: Heat::Cool }
    }

    fn strained(&self) -> bool {
        self.on_battery || self.saver
    }

    pub fn quality(&self, runtime: &Runtime) -> Quality {
        self.level(runtime).0
    }

    pub fn still(&self, runtime: &Runtime) -> bool {
        runtime.cost(Quality::Q0) > self.tier.budget()
    }

    fn chosen(&self, refresh: u32) -> u32 {
        match self.max_fps {
            DISPLAY if self.strained() => BATTERY,
            DISPLAY => display(refresh),
            fps => fps,
        }
    }

    pub fn rate(&self, refresh: u32, cost: Option<f32>) -> u32 {
        let rate = self.tier.max_fps(self.strained(), known(refresh, self.chosen(refresh)), cost);
        rate.min(self.heat.fps()).max(1)
    }

    pub fn video(&self, refresh: u32) -> u32 {
        let chosen = match self.max_fps {
            DISPLAY if self.tier == Tier::T0 => BATTERY,
            _ => self.chosen(refresh),
        };
        known(refresh, chosen).max(1)
    }

    pub fn fps(&self, runtime: &Runtime, quality: Quality, refresh: u32) -> u32 {
        let clock = match runtime.scene.clock.max_fps {
            DISPLAY => display(refresh),
            fps => fps,
        };
        let rate = self.rate(refresh, Some(runtime.cost(quality)));
        rate.min(clock).min(quality.max_fps()).min(runtime.pace()).max(1)
    }
}

#[cfg(test)]
mod tests {
    use crate::{Budget, Heat, Quality, Runtime, SCHEMA, Scene, Tier};
    use serde_json::json;
    use std::sync::Arc;

    fn runtime(layers: usize) -> Runtime {
        let draw = |i: usize| json!({ "id": format!("p{i}"), "type": "particles", "system": "dots" });
        let dots = json!({ "capacity": 200_000, "emitters": [{ "shape": "edge", "rate": 1000 }] });
        let layers: Vec<_> = (0..layers).map(draw).collect();
        let text = json!({ "schema": SCHEMA, "canvas": { "size": [960, 540] }, "layers": layers, "particles": { "dots": dots } });
        Runtime::new(Arc::new(Scene::from_json(&text.to_string()).unwrap()), Default::default())
    }

    #[test]
    fn scenes_over_budget_at_the_lowest_level_stay_still() {
        let power = |tier| Budget::new(tier, false, 60);
        assert!(!power(Tier::T0).still(&runtime(1)) && !power(Tier::T3).still(&runtime(16)));
        assert!(power(Tier::T0).still(&runtime(256)));
    }

    fn rate(tier: Tier, on_battery: bool, max_fps: u32, refresh: u32, cost: Option<f32>) -> u32 {
        Budget::new(tier, on_battery, max_fps).rate(refresh, cost)
    }

    #[test]
    fn the_display_rate_needs_mains_power_and_a_gpu_that_keeps_up() {
        let (light, heavy, display) = (Some(1.5), Some(12.0), 0);
        let mains =
            [(Tier::T0, light), (Tier::T2, light), (Tier::T2, heavy), (Tier::T2, None), (Tier::T3, heavy)];
        assert_eq!(mains.map(|(tier, cost)| rate(tier, false, display, 170, cost)), [24, 170, 60, 60, 170]);
        let tiers = [Tier::T0, Tier::T2, Tier::T3];
        assert_eq!(tiers.map(|tier| rate(tier, true, display, 170, light)), [15, 30, 30]);
        assert_eq!(tiers.map(|tier| rate(tier, true, 144, 170, light)), [15, 30, 60]);
        assert_eq!([300, 0, 60].map(|hz| rate(Tier::T3, false, display, hz, None)), [240, 60, 60]);
        assert_eq!(
            [(30, 60), (144, 60), (144, 0)].map(|(fps, hz)| rate(Tier::T3, false, fps, hz, None)),
            [30, 60, 144]
        );
        let pond = Some(8.5);
        assert_eq!(
            [(90, 144), (90, 90), (100, 0), (144, 144)].map(|(fps, hz)| rate(Tier::T2, false, fps, hz, pond)),
            [90, 90, 100, 60]
        );
    }

    #[test]
    fn videos_keep_the_chosen_rate_whatever_the_gpu() {
        let video =
            |tier, on_battery, max_fps, refresh| Budget::new(tier, on_battery, max_fps).video(refresh);
        assert_eq!([Tier::T0, Tier::T2, Tier::T3].map(|tier| video(tier, false, 0, 144)), [30, 144, 144]);
        assert_eq!([Tier::T0, Tier::T3].map(|tier| video(tier, true, 0, 144)), [30, 30]);
        assert_eq!(
            [(25, 0), (60, 0), (60, 24), (0, 0)].map(|(fps, hz)| video(Tier::T0, false, fps, hz)),
            [25, 60, 24, 30]
        );
        assert_eq!(video(Tier::T0, true, 60, 0), 60);
    }

    #[test]
    fn scenes_add_their_own_cap_and_follow_the_display_when_they_ask() {
        let power = Budget::new(Tier::T3, false, 0);
        let clocked = |max_fps| {
            let mut scene = (*runtime(1).scene).clone();
            scene.clock.max_fps = max_fps;
            Runtime::new(Arc::new(scene), Default::default())
        };
        let fps = |max_fps, quality| power.fps(&clocked(max_fps), quality, 120);
        assert_eq!([fps(0, Quality::Q3), fps(24, Quality::Q3), fps(0, Quality::Q2)], [120, 24, 60]);
    }

    #[test]
    fn phones_on_battery_get_a_lower_level_and_rate() {
        let light = runtime(0);
        let budget = |on_battery| Budget::new(Tier::T1, on_battery, 60);
        assert_eq!((budget(false).quality(&light), budget(true).quality(&light)), (Quality::Q2, Quality::Q1));
        assert_eq!(
            (budget(false).fps(&light, Quality::Q2, 0), budget(true).fps(&light, Quality::Q1, 0)),
            (30, 30)
        );
        let capped = Budget { max_fps: 10, ..budget(true) };
        assert_eq!(capped.fps(&light, Quality::Q0, 0), 10);
    }

    #[test]
    fn power_saving_and_heat_lower_the_level_and_the_rate() {
        let light = runtime(0);
        let mains = Budget::new(Tier::T3, false, 0);
        let saver = Budget { saver: true, ..mains };
        assert_eq!((mains.quality(&light), saver.quality(&light)), (Quality::Q3, Quality::Q1));
        assert_eq!((mains.rate(144, None), saver.rate(144, None), saver.video(144)), (144, 30, 30));
        let heat = |heat| Budget { heat, ..mains };
        let levels =
            [Heat::Cool, Heat::Warm, Heat::Hot].map(|h| (heat(h).quality(&light), heat(h).rate(144, None)));
        assert_eq!(levels, [(Quality::Q3, 144), (Quality::Q2, 30), (Quality::Q0, 15)]);
    }
}
