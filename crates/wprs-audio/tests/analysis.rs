use std::f32::consts::TAU;
use std::time::{Duration, Instant};
use wprs_audio::{Analyzer, AudioFrame};

const FPS: f32 = 60.0;
const KHZ_BAND: usize = 34;

struct Signal {
    analyzer: Analyzer,
    rate: u32,
    at: usize,
}

impl Signal {
    fn new(rate: u32) -> Signal {
        Signal { analyzer: Analyzer::new(rate), rate, at: 0 }
    }

    fn play(&mut self, seconds: f32, wave: impl Fn(f32) -> f32) -> Vec<AudioFrame> {
        let step = (self.rate as f32 / FPS) as usize;
        let frames = (seconds * FPS).round() as usize;
        (0..frames)
            .map(|_| {
                let t = |i: usize| (self.at + i) as f32 / self.rate as f32;
                let samples: Vec<f32> = (0..step).flat_map(|i| [wave(t(i)), wave(t(i))]).collect();
                self.at += step;
                self.analyzer.push(&samples);
                self.analyzer.update(1.0 / FPS).clone()
            })
            .collect()
    }
}

fn sine(t: f32) -> f32 {
    0.5 * (TAU * 1000.0 * t).sin()
}

fn loudest(bands: &[f32]) -> usize {
    (0..bands.len()).max_by(|&a, &b| bands[a].total_cmp(&bands[b])).unwrap()
}

#[test]
fn a_sine_peaks_in_its_band_at_any_rate() {
    for rate in [48_000, 44_100] {
        let mut signal = Signal::new(rate);
        let frames = signal.play(0.5, sine);
        let last = frames.last().unwrap();
        assert_eq!(loudest(&last.average), KHZ_BAND, "{rate}");
        assert!(last.average[KHZ_BAND] > 0.9 && last.left == last.right && !last.silent);
        assert!(last.mid > last.bass && last.mid > last.treble && last.level > 0.8, "{last:?}");
        assert!(frames.iter().all(|f| !f.beat || f.level < 0.5), "a steady tone has no beats");
    }
}

#[test]
fn silence_reads_zero_and_becomes_silent() {
    let mut signal = Signal::new(48_000);
    assert!(signal.play(0.5, |_| 0.0).iter().all(|f| f.silent && f.average.iter().all(|&v| v == 0.0)));
    assert!(!signal.play(0.5, sine).last().unwrap().silent);
    let after = signal.play(3.0, |_| 0.0);
    let last = after.last().unwrap();
    assert!(last.silent && last.level < 0.01 && last.average.iter().all(|&v| v < 0.01), "{last:?}");
    assert!(!after[60].silent, "silence counts only after two seconds");
}

#[test]
fn broken_samples_do_not_stick() {
    let mut signal = Signal::new(48_000);
    for bad in [f32::NAN, f32::INFINITY, 1e30] {
        signal.play(0.1, |_| bad);
    }
    let last = signal.play(1.0, sine).pop().unwrap();
    assert!(last.average[KHZ_BAND] > 0.9 && last.level.is_finite(), "{last:?}");
}

#[test]
fn bands_rise_fast_and_fall_slowly() {
    let mut signal = Signal::new(48_000);
    signal.play(0.5, |_| 0.0);
    let rise = signal.play(0.5, sine).iter().position(|f| f.average[KHZ_BAND] > 0.9).unwrap();
    assert!((rise as f32) / FPS < 0.1, "rose in {rise} frames");
    let fall = signal.play(2.0, |_| 0.0).iter().position(|f| f.average[KHZ_BAND] < 0.1).unwrap();
    assert!((fall as f32) / FPS > 0.3, "fell in {fall} frames");
}

#[test]
fn kicks_beat_with_their_period() {
    let kick = |t: f32| {
        let since = t % 0.5;
        (TAU * 80.0 * since).sin() * (-since / 0.03).exp()
    };
    let mut signal = Signal::new(48_000);
    let frames = signal.play(6.0, kick);
    let beats: Vec<f32> = (0..frames.len()).filter(|&i| frames[i].beat).map(|i| i as f32 / FPS).collect();
    assert!((11..=12).contains(&beats.len()), "{beats:?}");
    assert!(beats.windows(2).all(|w| (w[1] - w[0] - 0.5).abs() < 0.05), "{beats:?}");
    assert!(frames.iter().all(|f| f.bass >= f.treble), "kicks are bass");
}

#[test]
fn analysis_fits_the_frame_budget() {
    let mut analyzer = Analyzer::new(48_000);
    let samples: Vec<f32> = (0..1600).map(|i| sine(i as f32 / 96_000.0)).collect();
    let mut batch = || {
        let start = Instant::now();
        for _ in 0..24 {
            analyzer.push(&samples);
            analyzer.update(1.0 / FPS);
        }
        start.elapsed() / 24
    };
    let per_frame = (0..5).map(|_| batch()).min().unwrap();
    assert!(per_frame < Duration::from_millis(2), "{per_frame:?} per frame");
}

#[test]
fn a_spectrum_lands_in_the_same_bands_as_samples() {
    let mut analyzer = Analyzer::new(48_000);
    let mut bins = vec![0.0; 513];
    bins[(1000.0 * 1024.0 / 44_100.0_f32).round() as usize] = 0.25;
    let mut last = AudioFrame::default();
    for _ in 0..30 {
        analyzer.push_spectrum(&bins, 44_100);
        last = analyzer.update(1.0 / FPS).clone();
    }
    assert_eq!(loudest(&last.average), KHZ_BAND);
    assert!(last.left == last.right && !last.silent && last.mid > last.bass, "{last:?}");
    for _ in 0..200 {
        analyzer.push_spectrum(&[0.0; 513], 44_100);
        last = analyzer.update(1.0 / FPS).clone();
    }
    assert!(last.silent && last.average.iter().all(|&v| v < 0.01), "{last:?}");
    analyzer.push_spectrum(&[], 44_100);
    analyzer.push_spectrum(&[f32::NAN; 3], 0);
    assert!(analyzer.update(0.5).average.iter().all(|v| v.is_finite()));
}
