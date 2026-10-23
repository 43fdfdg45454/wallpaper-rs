use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};
use wprs_audio::{Analyzer, AudioFrame};

pub(crate) const MAX_CAPTURE: usize = 1024;
const BINS: usize = MAX_CAPTURE / 2 + 1;
const LIVE: Duration = Duration::from_secs(2);
const FLOOR: f32 = 4.0 / 16_384.0;

struct Hearing {
    analyzer: Analyzer,
    shown: Option<Instant>,
    heard: Option<Instant>,
}

static HEARING: Mutex<Option<Hearing>> = Mutex::new(None);
static PLAYING: AtomicBool = AtomicBool::new(false);

fn lock() -> MutexGuard<'static, Option<Hearing>> {
    HEARING.lock().unwrap_or_else(PoisonError::into_inner)
}

fn spectrum(fft: &[i8], bins: &mut [f32; BINS]) -> usize {
    let n = fft.len().min(MAX_CAPTURE) & !1;
    if n < 4 {
        return 0;
    }
    let level = |v: i8| (f32::from(v) / 128.0).powi(2);
    (bins[0], bins[n / 2]) = (level(fft[0]), level(fft[1]));
    for (k, bin) in bins.iter_mut().enumerate().take(n / 2).skip(1) {
        *bin = level(fft[2 * k]) + level(fft[2 * k + 1]);
    }
    n / 2 + 1
}

pub(crate) fn hear(fft: &[i8], rate: u32) {
    let mut bins = [0.0; BINS];
    let n = spectrum(fft, &mut bins);
    if n > 0 {
        let mut hearing = lock();
        let fresh = || Hearing { analyzer: Analyzer::new(rate), shown: None, heard: None };
        let hearing = hearing.get_or_insert_with(fresh);
        if bins[..n].iter().any(|&bin| bin > FLOOR) {
            hearing.heard = Some(Instant::now());
        }
        hearing.analyzer.push_spectrum(&bins[..n], rate);
    }
}

pub(crate) fn playback(active: bool) {
    PLAYING.store(active, Relaxed);
}

pub(crate) fn playing() -> bool {
    PLAYING.load(Relaxed)
}

pub(crate) fn live() -> bool {
    lock().as_ref().and_then(|hearing| hearing.heard).is_some_and(|heard| heard.elapsed() < LIVE)
}

pub(crate) fn frame() -> AudioFrame {
    let mut hearing = lock();
    let Some(Hearing { analyzer, shown, .. }) = hearing.as_mut() else { return AudioFrame::default() };
    let now = Instant::now();
    let dt = shown.replace(now).map_or(0.0, |last| now.duration_since(last).as_secs_f32());
    analyzer.update(dt).clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_visualizer_spectrum_lights_its_band_and_fades_to_silence() {
        let (mut analyzer, mut bins, mut fft) = (Analyzer::new(44_100), [0.0; BINS], [0i8; MAX_CAPTURE]);
        fft[2 * 23] = 100;
        assert_eq!((spectrum(&fft, &mut bins), spectrum(&[0; 3], &mut bins)), (BINS, 0));
        let mut heard = AudioFrame::default();
        for _ in 0..20 {
            spectrum(&fft, &mut bins);
            analyzer.push_spectrum(&bins, 44_100);
            heard = analyzer.update(0.03).clone();
        }
        let band = (0..heard.average.len()).max_by(|&a, &b| heard.average[a].total_cmp(&heard.average[b]));
        assert!(!heard.silent && heard.mid > heard.bass && band == Some(34), "{heard:?}");
        spectrum(&[0; MAX_CAPTURE], &mut bins);
        for _ in 0..40 {
            analyzer.push_spectrum(&bins, 44_100);
            heard = analyzer.update(0.06).clone();
        }
        assert!(heard.silent && heard.average.iter().all(|&v| v < 0.01), "{heard:?}");
    }
}
