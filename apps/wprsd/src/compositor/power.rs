use wprs_scene::{Quality, Runtime, Tier};

#[derive(Debug, Clone, Copy)]
pub(super) struct Power {
    pub(super) tier: Tier,
    pub(super) on_battery: bool,
    pub(super) max_fps: u32,
}

impl Power {
    pub(super) fn quality(&self, runtime: &Runtime) -> Quality {
        Quality::fit(&runtime.scene, self.tier, self.tier.quality(self.on_battery))
    }

    pub(super) fn still(&self, runtime: &Runtime) -> bool {
        runtime.scene.cost(Quality::Q0) > self.tier.budget()
    }

    pub(super) fn fps(&self, runtime: &Runtime, quality: Quality) -> u32 {
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
    use super::Power;
    use serde_json::json;
    use std::sync::Arc;
    use wprs_scene::{Runtime, SCHEMA, Scene, Tier};

    fn runtime(layers: usize) -> Runtime {
        let draw = |i: usize| json!({ "id": format!("p{i}"), "type": "particles", "system": "dots" });
        let dots = json!({ "capacity": 200_000, "emitters": [{ "shape": "edge", "rate": 1000 }] });
        let layers: Vec<_> = (0..layers).map(draw).collect();
        let text = json!({ "schema": SCHEMA, "canvas": { "size": [960, 540] }, "layers": layers, "particles": { "dots": dots } });
        Runtime::new(Arc::new(Scene::from_json(&text.to_string()).unwrap()), Default::default())
    }

    #[test]
    fn scenes_over_budget_at_the_lowest_level_stay_still() {
        let power = |tier| Power { tier, on_battery: false, max_fps: 60 };
        assert!(!power(Tier::T0).still(&runtime(1)) && !power(Tier::T3).still(&runtime(16)));
        assert!(power(Tier::T0).still(&runtime(256)));
    }
}
