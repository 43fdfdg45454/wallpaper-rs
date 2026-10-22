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
