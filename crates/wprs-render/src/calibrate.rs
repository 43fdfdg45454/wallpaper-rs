use crate::{Layer, RenderError, Renderer};
use std::path::Path;
use std::time::{Duration, Instant};
use wprs_core::Fit;
use wprs_scene::{Calibration, Tier};

const SIZE: (u32, u32) = (1920, 1080);
const PASSES: usize = 16;
const SPAN: Duration = Duration::from_millis(300);
const FRAMES: usize = 12;

impl Renderer {
    pub fn tier(&self) -> Tier {
        self.calibration
            .get()
            .map_or_else(|| *self.tier.get_or_init(|| Tier::detect(&self.gpu_class())), |c| c.tier)
    }

    pub fn calibration(&self) -> Option<&Calibration> {
        self.calibration.get()
    }

    pub fn calibrate(&self, cache: Option<&Path>) -> &Calibration {
        self.calibration.get_or_init(|| {
            let info = self.gpu.get_info();
            let adapter = format!("{} {:04x}:{:04x} {:?}", info.name, info.vendor, info.device, info.backend);
            let driver = format!("{} {}", info.driver, info.driver_info);
            let measure = || self.unit_ms(SIZE, PASSES).inspect_err(|e| tracing::warn!("{e}")).ok();
            let (found, fresh) =
                wprs_scene::calibrate(cache, (&adapter, &driver), &self.gpu_class(), measure);
            if let Some(path) = cache.filter(|_| fresh)
                && let Err(e) = found.keep(path)
            {
                tracing::warn!("cannot keep the graphics card class in {}: {e}", path.display());
            }
            tracing::debug!(tier = ?found.tier, unit_ms = found.unit_ms, fresh, "graphics card class");
            found
        })
    }

    pub fn unit_ms(&self, (width, height): (u32, u32), passes: usize) -> Result<f64, RenderError> {
        let texel =
            |i: u32| [(i % width * 7) as u8, (i / width * 13) as u8, (i.wrapping_mul(31) >> 3) as u8, 255];
        let pixels: Vec<u8> = (0..width * height).flat_map(texel).collect();
        let image = self.upload_rgba(&pixels, width, height, width * 4)?;
        drop(pixels);
        let mut target = self.create_target(width, height)?;
        let layer = |i: usize| Layer {
            image: Some(&image),
            fit: Fit::Fill,
            background: [0; 3],
            opacity: if i == 0 { 1.0 } else { 0.5 },
            uv: None,
            relief: None,
        };
        let layers: Vec<Layer> = (0..passes.max(1)).map(layer).collect();
        let (start, mut times) = (Instant::now(), Vec::new());
        for i in 0..FRAMES + 2 {
            let at = Instant::now();
            self.draw_layers(&mut target, &layers)?;
            self.finish()?;
            times.extend((i >= 2).then(|| at.elapsed().as_secs_f64() * 1e3));
            if times.len() >= 4 && start.elapsed() > SPAN {
                break;
            }
        }
        times.sort_by(f64::total_cmp);
        Ok(times[times.len() / 4] / layers.len() as f64)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_unit_of_work_is_timed_on_the_gpu() {
        let r = match crate::Renderer::shared() {
            Ok(r) => r,
            Err(e) => return assert!(std::env::var_os("WPRS_REQUIRE_GPU").is_none(), "GPU required: {e}"),
        };
        let ms = r.unit_ms((64, 36), 2).unwrap();
        assert!(ms.is_finite() && ms > 0.0, "{ms}");
    }
}
