use crate::{Quality, Runtime, Tier};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    pub tier: Tier,
    pub on_battery: bool,
    pub max_fps: u32,
}

impl Budget {
    pub fn quality(&self, runtime: &Runtime) -> Quality {
        Quality::fit(&runtime.scene, self.tier, self.tier.quality(self.on_battery))
    }

    pub fn still(&self, runtime: &Runtime) -> bool {
        runtime.scene.cost(Quality::Q0) > self.tier.budget()
    }

    pub fn fps(&self, runtime: &Runtime, quality: Quality) -> u32 {
        let caps = [
            self.max_fps,
            runtime.scene.clock.max_fps,
            quality.max_fps(),
            self.tier.max_fps(self.on_battery),
        ];
        caps.into_iter().min().unwrap_or(1).max(1)
    }
}

#[cfg(test)]
mod tests {
    use crate::{Budget, Quality, Runtime, SCHEMA, Scene, Tier};
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
        let budget = |tier| Budget { tier, on_battery: false, max_fps: 60 };
        assert!(!budget(Tier::T0).still(&runtime(1)) && !budget(Tier::T3).still(&runtime(16)));
        assert!(budget(Tier::T0).still(&runtime(256)));
    }

    #[test]
    fn phones_on_battery_get_a_lower_level_and_rate() {
        let light = runtime(0);
        let budget = |on_battery| Budget { tier: Tier::T1, on_battery, max_fps: 60 };
        assert_eq!((budget(false).quality(&light), budget(true).quality(&light)), (Quality::Q2, Quality::Q1));
        assert_eq!((budget(false).fps(&light, Quality::Q2), budget(true).fps(&light, Quality::Q1)), (30, 30));
        let capped = Budget { max_fps: 10, ..budget(true) };
        assert_eq!(capped.fps(&light, Quality::Q0), 10);
    }
}
