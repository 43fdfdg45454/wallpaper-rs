use std::time::Instant;
use wprs_render::{GpuImage, Meter, RenderError, Renderer};
use wprs_scene::{Calming, FlashGuard};

pub(super) struct Guard {
    guard: FlashGuard,
    luma: Vec<f32>,
    meter: Option<Meter>,
    since: Instant,
}

impl Guard {
    pub(super) fn new(now: Instant) -> Box<Guard> {
        Box::new(Guard { guard: FlashGuard::default(), luma: Vec::new(), meter: None, since: now })
    }

    pub(super) fn watch(
        &mut self,
        r: &Renderer,
        image: Option<&GpuImage>,
        now: Instant,
        scene: &str,
    ) -> Result<(), RenderError> {
        let meter = self.meter.get_or_insert_with(|| r.meter());
        if !r.luma(meter, image, &mut self.luma)? {
            return Ok(());
        }
        let before = self.guard.calming();
        let after = self.guard.watch(now.duration_since(self.since).as_secs_f64(), &self.luma);
        match after {
            _ if after == before => {}
            Calming::Slowed(speed) => {
                tracing::warn!(
                    scene,
                    speed,
                    "the scene flashed more than three times a second and plays slower"
                )
            }
            Calming::Paused => {
                tracing::warn!(scene, "the scene kept flashing more than three times a second and stopped")
            }
            Calming::Steady => {}
        }
        Ok(())
    }

    pub(super) fn speed(&self) -> f64 {
        self.guard.speed()
    }

    pub(super) fn label(&self) -> &'static str {
        match self.guard.calming() {
            Calming::Steady => "",
            Calming::Slowed(_) => "slowed",
            Calming::Paused => "paused",
        }
    }
}

#[cfg(test)]
mod tests;
