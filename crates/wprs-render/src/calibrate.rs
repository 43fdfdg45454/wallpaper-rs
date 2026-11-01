use crate::{Layer, RenderError, Renderer};
use std::path::Path;
use std::sync::atomic::Ordering::Relaxed;
use std::time::{Duration, Instant};
use wprs_core::Fit;
use wprs_scene::{Calibration, Tier};

struct Stamps {
    set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    read: wgpu::Buffer,
    period: f64,
}

impl Stamps {
    fn new(r: &Renderer) -> Stamps {
        let ty = wgpu::QueryType::Timestamp;
        let set = r.device.create_query_set(&wgpu::QuerySetDescriptor { label: None, ty, count: 2 });
        let buffer = |usage| {
            r.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: 16,
                usage,
                mapped_at_creation: false,
            })
        };
        let resolve = buffer(wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC);
        let read = buffer(wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ);
        Stamps { set, resolve, read, period: f64::from(r.queue.get_timestamp_period()) }
    }

    fn mark(&self, r: &Renderer, end: bool) -> wgpu::CommandEncoder {
        let mut encoder = r.device.create_command_encoder(&Default::default());
        let timestamp_writes = Some(wgpu::ComputePassTimestampWrites {
            query_set: &self.set,
            beginning_of_pass_write_index: (!end).then_some(0),
            end_of_pass_write_index: end.then_some(1),
        });
        drop(encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None, timestamp_writes }));
        encoder
    }

    fn time(
        &self,
        r: &Renderer,
        target: &mut crate::Target,
        layers: &[Layer<'_>],
    ) -> Result<Option<f64>, RenderError> {
        let size = target.size();
        r.scoped(|| {
            let drawn =
                r.encode(&target.view, &mut target.slots, layers, size, crate::draw::TARGET_FORMAT, None);
            let mut end = self.mark(r, true);
            end.resolve_query_set(&self.set, 0..2, &self.resolve, 0);
            end.copy_buffer_to_buffer(&self.resolve, 0, &self.read, 0, 16);
            r.queue.submit([self.mark(r, false).finish(), drawn.finish(), end.finish()]);
        })?;
        let slice = self.read.slice(..);
        let mapped = std::sync::Arc::new(std::sync::atomic::AtomicU8::new(0));
        let flag = mapped.clone();
        slice.map_async(wgpu::MapMode::Read, move |done| flag.store(1 + u8::from(done.is_ok()), Relaxed));
        r.finish()?;
        let ms = (mapped.load(Relaxed) == 2).then(|| slice.get_mapped_range().ok()).flatten().map(|data| {
            let tick = |at: usize| u64::from_le_bytes(data[at..at + 8].try_into().unwrap_or_default());
            tick(8).saturating_sub(tick(0)) as f64 * self.period / 1e6
        });
        self.read.unmap();
        Ok(ms)
    }
}

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
            let method = if self.timestamps() { Calibration::STAMPS } else { Calibration::FENCE };
            let (found, fresh) =
                wprs_scene::calibrate(cache, (&adapter, &driver), &self.gpu_class(), method, measure);
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
            flip: [false; 2],
            grade: None,
        };
        let layers: Vec<Layer> = (0..passes.max(1)).map(layer).collect();
        let (start, mut times) = (Instant::now(), Vec::new());
        let stamps = self.timestamps().then(|| Stamps::new(self));
        for i in 0..FRAMES + 2 {
            let at = Instant::now();
            let gpu = match &stamps {
                Some(stamps) => stamps.time(self, &mut target, &layers)?,
                None => {
                    self.draw_layers(&mut target, &layers)?;
                    None
                }
            };
            self.finish()?;
            let wall = at.elapsed().as_secs_f64() * 1e3;
            times.extend((i >= 2).then(|| gpu.filter(|ms| *ms > 0.0).unwrap_or(wall)));
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
        let method = r.calibrate(None).method;
        let wanted =
            if r.timestamps() { wprs_scene::Calibration::STAMPS } else { wprs_scene::Calibration::FENCE };
        assert_eq!(method, wanted, "the calibration says how it was timed");
    }
}
