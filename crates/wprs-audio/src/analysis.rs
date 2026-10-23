use realfft::num_complex::Complex;
use realfft::{RealFftPlanner, RealToComplex};
use std::f32::consts::TAU;
use std::sync::Arc;
use wprs_scene::{AudioFrame, BANDS};

pub const WINDOW: usize = 2048;
const RANGE: (f32, f32) = (40.0, 16_000.0);
const SPLITS: (f32, f32) = (250.0, 4_000.0);
const FLOOR_DB: f32 = -70.0;
const ATTACK: f32 = 0.02;
const RELEASE: f32 = 0.2;
const PEAK_DECAY: f32 = 5.0;
const MIN_PEAK: f32 = 1e-4;
const QUIET: f32 = 1e-6;
const SILENT_AFTER: f32 = 2.0;
const STALE: f32 = 0.1;
const LOCKOUT: f32 = 0.12;

pub fn smooth(v: f32, target: f32, dt: f32, up: f32, down: f32) -> f32 {
    let tau = if target > v { up } else { down };
    v + (target - v) * (1.0 - (-dt / tau).exp())
}

pub fn db01(power: f32, floor_db: f32) -> f32 {
    ((10.0 * power.max(1e-12).log10() - floor_db) / -floor_db).clamp(0.0, 1.0)
}

pub struct Analyzer {
    fft: Arc<dyn RealToComplex<f32>>,
    hann: Vec<f32>,
    history: [Vec<f32>; 2],
    head: usize,
    input: Vec<f32>,
    spectrum: Vec<Complex<f32>>,
    scratch: Vec<Complex<f32>>,
    bands: [(usize, usize, f32); BANDS],
    centers: [f32; BANDS],
    rate: u32,
    raw: [[f32; BANDS]; 2],
    last: [f32; BANDS],
    power: f32,
    peak: f32,
    flux: (f32, f32),
    since_beat: f32,
    quiet: f32,
    stale: f32,
    fresh: bool,
    given: bool,
    frame: AudioFrame,
}

impl Analyzer {
    pub fn new(rate: u32) -> Analyzer {
        let fft = RealFftPlanner::new().plan_fft_forward(WINDOW);
        let hann = (0..WINDOW).map(|i| 0.5 - 0.5 * (TAU * i as f32 / WINDOW as f32).cos()).collect();
        let (input, spectrum, scratch) =
            (fft.make_input_vec(), fft.make_output_vec(), fft.make_scratch_vec());
        let (bands, centers) = layout(rate, WINDOW);
        let (raw, last, frame) = ([[0.0; BANDS]; 2], [0.0; BANDS], AudioFrame::default());
        let history = [vec![0.0; WINDOW], vec![0.0; WINDOW]];
        let quiet = SILENT_AFTER;
        Analyzer {
            fft,
            hann,
            history,
            head: 0,
            input,
            spectrum,
            scratch,
            bands,
            centers,
            rate,
            raw,
            last,
            power: 0.0,
            peak: MIN_PEAK,
            flux: (0.0, 0.0),
            since_beat: LOCKOUT,
            quiet,
            stale: 0.0,
            fresh: false,
            given: false,
            frame,
        }
    }

    pub fn set_rate(&mut self, rate: u32) {
        if rate != self.rate && rate > 0 {
            ((self.bands, self.centers), self.rate) = (layout(rate, WINDOW), rate);
        }
    }

    pub fn push(&mut self, stereo: &[f32]) {
        let clean = |x: f32| if x.is_finite() { x.clamp(-1.0, 1.0) } else { 0.0 };
        for &[left, right] in stereo.as_chunks::<2>().0 {
            (self.history[0][self.head], self.history[1][self.head]) = (clean(left), clean(right));
            self.head = (self.head + 1) % WINDOW;
            self.fresh = true;
        }
    }

    pub fn push_spectrum(&mut self, power: &[f32], rate: u32) {
        let window = 2 * power.len().saturating_sub(1);
        if window == 0 || rate == 0 {
            return;
        }
        let bin = |k: usize| power.get(k).copied().filter(|p| p.is_finite()).unwrap_or(0.0).clamp(0.0, 1.0);
        let bands = layout(rate, window).0.map(|band| level(band, bin));
        let total = (0..power.len()).map(bin).sum::<f32>() / 2.0;
        (self.raw, self.power, self.given, self.fresh) = ([bands; 2], total.min(1.0), true, true);
    }

    pub fn update(&mut self, dt: f32) -> &AudioFrame {
        let dt = dt.clamp(0.0, 1.0);
        self.stale = if self.fresh { 0.0 } else { self.stale + dt };
        if self.stale > STALE && self.power > 0.0 {
            self.history.iter_mut().for_each(|h| h.fill(0.0));
            self.fresh = true;
        }
        let fresh = std::mem::take(&mut self.fresh);
        if fresh && !std::mem::take(&mut self.given) {
            self.raw = [self.analyze(0), self.analyze(1)];
            self.power = self.history.iter().flatten().map(|x| x * x).sum::<f32>() / (2 * WINDOW) as f32;
        }
        let loudest = self.raw.iter().flatten().fold(0.0, |a: f32, &b| a.max(b));
        self.peak = loudest.max(self.peak * (-dt / PEAK_DECAY).exp()).max(MIN_PEAK);
        let (f, mut flux) = (&mut self.frame, 0.0);
        for b in 0..BANDS {
            let [l, r] = self.raw.map(|c| db01(c[b] / self.peak, FLOOR_DB));
            if fresh && self.centers[b] < SPLITS.0 {
                flux += ((l + r) / 2.0 - self.last[b]).max(0.0);
            }
            if fresh {
                self.last[b] = (l + r) / 2.0;
            }
            f.left[b] = smooth(f.left[b], l, dt, ATTACK, RELEASE);
            f.right[b] = smooth(f.right[b], r, dt, ATTACK, RELEASE);
            f.average[b] = (f.left[b] + f.right[b]) / 2.0;
        }
        let mean = |lo: f32, hi: f32| {
            let within = (0..BANDS).filter(|&b| (lo..hi).contains(&self.centers[b]));
            let (sum, n) = within.fold((0.0, 0), |(s, n), b| (s + f.average[b], n + 1));
            sum / n.max(1) as f32
        };
        (f.bass, f.mid, f.treble) = (mean(0.0, SPLITS.0), mean(SPLITS.0, SPLITS.1), mean(SPLITS.1, f32::MAX));
        f.level = smooth(f.level, db01(self.power, FLOOR_DB), dt, ATTACK, RELEASE);
        let (avg, var) = self.flux;
        f.beat = fresh && flux > 0.05 && flux > avg + 1.5 * var.sqrt() && self.since_beat >= LOCKOUT;
        if fresh {
            self.flux = (avg + 0.1 * (flux - avg), 0.9 * (var + 0.1 * (flux - avg).powi(2)));
        }
        self.since_beat = if f.beat { 0.0 } else { self.since_beat + dt };
        self.quiet = if self.power < QUIET { self.quiet + dt } else { 0.0 };
        f.silent = self.quiet >= SILENT_AFTER;
        &self.frame
    }

    fn analyze(&mut self, channel: usize) -> [f32; BANDS] {
        let history = &self.history[channel];
        for (i, x) in self.input.iter_mut().enumerate() {
            *x = history[(self.head + i) % WINDOW] * self.hann[i];
        }
        let _ = self.fft.process_with_scratch(&mut self.input, &mut self.spectrum, &mut self.scratch);
        let scale = (WINDOW as f32 / 4.0).powi(-2);
        let power = |k: usize| self.spectrum.get(k).map_or(0.0, |x| x.norm_sqr() * scale);
        self.bands.map(|band| level(band, power))
    }
}

fn level((lo, hi, f): (usize, usize, f32), power: impl Fn(usize) -> f32) -> f32 {
    match lo < hi {
        true => (lo..hi).map(power).fold(0.0, f32::max),
        false => power(lo) * (1.0 - f) + power(lo + 1) * f,
    }
}

fn layout(rate: u32, window: usize) -> ([(usize, usize, f32); BANDS], [f32; BANDS]) {
    let bin = rate as f32 / window as f32;
    let edge = |i: usize| RANGE.0 * (RANGE.1 / RANGE.0).powf(i as f32 / BANDS as f32);
    let band = |i: usize| {
        let (lo, hi) = (edge(i) / bin, edge(i + 1) / bin);
        let (a, b, middle) = (lo.ceil() as usize, hi.ceil() as usize, (lo + hi) / 2.0);
        if a < b { (a, b, 0.0) } else { (middle as usize, middle as usize, middle.fract()) }
    };
    (std::array::from_fn(band), std::array::from_fn(|i| (edge(i) * edge(i + 1)).sqrt()))
}
