use super::SceneState;
use crate::compositor::power::Power;
use std::time::Instant;
use wprs_scene::Quality;

impl SceneState {
    pub(super) fn level(&mut self, power: Power) -> Quality {
        let (quality, why) = self.governor.level(power.level(&self.runtime));
        self.why = why;
        quality.min(self.spec.pack.cap.unwrap_or(Quality::Q3))
    }

    pub(super) fn judge(&mut self, (samples, gpu_ms): (u64, f64), power: Power, refresh: u32, now: Instant) {
        if std::mem::replace(&mut self.samples, samples) == samples || self.still {
            return;
        }
        let runtime = &self.runtime;
        let budget =
            |q| (1000.0 / power.fps(runtime, q, refresh).max(1) as f32 * power.gpu_share(), runtime.cost(q));
        let ceiling = power.level(runtime).0;
        if self.governor.feed(now, gpu_ms as f32, budget, ceiling, power.raises()) {
            let level = self.governor.level((ceiling, self.why)).0;
            tracing::debug!(?level, gpu_ms, "the GPU time of the frames changed the scene quality");
        }
    }

    pub(in super::super) fn quality(&self) -> (&'static str, &'static str) {
        (["q0", "q1", "q2", "q3"][self.quality as usize], self.why.name())
    }
}
