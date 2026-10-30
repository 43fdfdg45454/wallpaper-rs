use super::SceneState;
use crate::compositor::power::Power;
use std::time::Instant;
use wprs_scene::Quality;

impl SceneState {
    pub(super) fn level(&mut self, power: Power) -> Quality {
        let (quality, why) = self.governor.level(power.level(&self.runtime));
        self.why = why;
        quality
    }

    pub(super) fn judge(&mut self, (samples, gpu_ms): (u64, f64), power: Power, fps: u32, now: Instant) {
        if std::mem::replace(&mut self.samples, samples) == samples || self.still {
            return;
        }
        let budget = 1000.0 / fps.max(1) as f32 * power.gpu_share();
        let ceiling = power.level(&self.runtime).0;
        if self.governor.feed(now, gpu_ms as f32, budget, ceiling, power.raises()) {
            let level = self.governor.level((ceiling, self.why)).0;
            tracing::debug!(?level, gpu_ms, budget, "the GPU time of the frames changed the scene quality");
        }
    }

    pub(in super::super) fn quality(&self) -> (&'static str, &'static str) {
        (["q0", "q1", "q2", "q3"][self.quality as usize], self.why.name())
    }
}
