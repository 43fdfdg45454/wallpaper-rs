use crate::Renderer;
use crate::readback::GPU_TIMEOUT;
use std::sync::OnceLock;
use std::time::Duration;

const PIXELS_PER_CORE: f64 = 1_048_576.0;
const SLOWEST: u32 = 6;
const SMALLEST: f32 = 0.25;
const STEP: f32 = 0.7;
const SETTLE: u32 = 3;

pub(crate) fn cores() -> u32 {
    static CORES: OnceLock<u32> = OnceLock::new();
    *CORES.get_or_init(|| {
        let threads = std::env::var("LP_NUM_THREADS").ok().and_then(|v| v.parse::<u32>().ok());
        let found = || std::thread::available_parallelism().map_or(1, |n| n.get() as u32);
        threads.filter(|n| *n > 0).unwrap_or_else(found).max(1)
    })
}

pub(crate) fn timeout_for(software: bool, cores: u32) -> Duration {
    match software {
        true => GPU_TIMEOUT * (32 / cores.max(1)).clamp(1, SLOWEST),
        false => GPU_TIMEOUT,
    }
}

pub(crate) fn scale_for(software: bool, cores: u32, (width, height): (u32, u32)) -> f32 {
    let pixels = f64::from(width.max(1)) * f64::from(height.max(1));
    match software {
        true => (f64::from(cores) * PIXELS_PER_CORE / pixels).sqrt().min(1.0) as f32,
        false => 1.0,
    }
}

pub(crate) fn shrink((scale, wait): (f32, u32), gpu_ms: f64, timeout: Duration) -> (f32, u32) {
    let limit = timeout.as_secs_f64() * 1000.0 / 8.0;
    match wait {
        0 if gpu_ms > limit && scale > SMALLEST => ((scale * STEP).max(SMALLEST), SETTLE),
        0 if gpu_ms > 0.0 && gpu_ms < limit / 16.0 && scale < 1.0 => ((scale / STEP).min(1.0), SETTLE),
        wait => (scale, wait.saturating_sub(1)),
    }
}

impl Renderer {
    pub fn gpu_timeout(&self) -> Duration {
        timeout_for(self.software(), cores())
    }

    pub fn render_scale(&self, size: (u32, u32)) -> f32 {
        scale_for(self.software(), cores(), size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn software_rendering_waits_longer_and_draws_fewer_pixels_with_fewer_cores() {
        assert_eq!(timeout_for(false, 2), GPU_TIMEOUT);
        let seconds = |cores| timeout_for(true, cores).as_secs();
        assert_eq!([2, 4, 8, 16, 32, 64].map(seconds), [60, 60, 40, 20, 10, 10]);
        assert_eq!(scale_for(false, 1, (7680, 4320)), 1.0);
        assert_eq!(scale_for(true, 2, (1920, 1080)), 1.0);
        let eight_k = scale_for(true, 2, (7680, 4320));
        assert!((eight_k - 0.25).abs() < 0.01, "{eight_k}");
        let pixels = |cores| (7680.0 * scale_for(true, cores, (7680, 4320))).powi(2) * 9.0 / 16.0;
        assert!([2, 8, 16].iter().all(|&c| pixels(c) <= f32::from(c as u16) * 1_048_576.0 * 1.01));
    }

    #[test]
    fn slow_frames_shrink_the_render_long_before_the_timeout_and_grow_back_slowly() {
        let timeout = Duration::from_secs(60);
        let mut state = (1.0, 0);
        state = shrink(state, 9000.0, timeout);
        assert_eq!(state, (STEP, SETTLE));
        for _ in 0..SETTLE {
            state = shrink(state, 9000.0, timeout);
        }
        assert_eq!(state.0, STEP, "frames drawn before the change do not count");
        while state.0 > SMALLEST {
            state = shrink(state, 9000.0, timeout);
        }
        assert_eq!(shrink((SMALLEST, 0), 9000.0, timeout), (SMALLEST, 0));
        assert_eq!(shrink((0.5, 0), 2000.0, timeout), (0.5, 0), "between the limits nothing changes");
        let (grown, _) = shrink((0.5, 0), 100.0, timeout);
        assert!((grown - 0.5 / STEP).abs() < 1e-6);
        assert_eq!(shrink((1.0, 0), 100.0, timeout), (1.0, 0));
    }
}
