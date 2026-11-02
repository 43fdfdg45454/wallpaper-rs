use super::SceneState;
use crate::compositor::Level;
use crate::compositor::power::Power;
use std::time::Instant;
use wprs_scene::Quality;

impl SceneState {
    pub(super) fn own(&self, power: Power) -> Power {
        Power { choice: self.spec.quality.unwrap_or(power.choice), ..power }
    }

    pub(super) fn level(&mut self, power: Power) -> Quality {
        let power = self.own(power);
        self.governor.hold(power.floor());
        let (quality, why) = self.governor.level(power.level(&self.runtime));
        self.why = why;
        let quality = quality.min(self.spec.pack.cap.unwrap_or(Quality::Q3));
        self.over = power.over(&self.runtime, quality);
        quality
    }

    pub(super) fn judge(&mut self, (samples, gpu_ms): (u64, f64), power: Power, refresh: u32, now: Instant) {
        if std::mem::replace(&mut self.samples, samples) == samples || self.still {
            return;
        }
        let (power, runtime) = (self.own(power), &self.runtime);
        let budget =
            |q| (1000.0 / power.fps(runtime, q, refresh).max(1) as f32 * power.gpu_share(), runtime.cost(q));
        let ceiling = power.level(runtime).0;
        if self.governor.feed(now, gpu_ms as f32, budget, ceiling, power.raises()) {
            let level = self.governor.level((ceiling, self.why)).0;
            tracing::debug!(?level, gpu_ms, "the GPU time of the frames changed the scene quality");
        }
    }

    pub(in super::super) fn quality(&self) -> Level {
        let (cost, budget) = self.over.unwrap_or_default();
        let quality = ["q0", "q1", "q2", "q3"][self.quality as usize];
        Level { quality, reason: self.why.name(), cost, budget }
    }
}
