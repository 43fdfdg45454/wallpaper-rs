use std::path::Path;
use std::time::{Duration, Instant};
use wprs_mix::{Clip, RATE};
use wprs_scene::MAX_SOUND_SECONDS;

#[cfg(target_os = "android")]
mod droid;
#[cfg(not(target_os = "android"))]
mod wav;

#[cfg(target_os = "android")]
pub(super) use droid::Decoder;
#[cfg(not(target_os = "android"))]
pub(super) use wav::Decoder;

const MARGIN: f32 = 0.5;
const WAIT: Duration = Duration::from_secs(30);

pub(super) struct Chunk {
    pub(super) rate: u32,
    pub(super) channels: usize,
    pub(super) start_us: i64,
    pub(super) samples: Vec<i16>,
}

#[derive(Default)]
pub(super) struct Resample {
    pos: f64,
    last: [f32; 2],
}

impl Resample {
    pub(super) fn run(&mut self, chunk: &Chunk, out: &mut Vec<i16>) {
        let (c, n) = (chunk.channels.clamp(1, 2), chunk.samples.len() / chunk.channels.max(1));
        if chunk.rate == RATE || n == 0 {
            out.extend_from_slice(&chunk.samples);
            return;
        }
        let step = f64::from(chunk.rate) / f64::from(RATE);
        let at = |i: isize, ch: usize| match usize::try_from(i) {
            Ok(i) => f32::from(chunk.samples[i * c + ch]),
            Err(_) => self.last[ch],
        };
        let mut pos = self.pos;
        while pos < (n - 1) as f64 {
            let (i, f) = (pos.floor(), (pos - pos.floor()) as f32);
            out.extend((0..c).map(|ch| (at(i as isize, ch) * (1.0 - f) + at(i as isize + 1, ch) * f) as i16));
            pos += step;
        }
        self.pos = pos - n as f64;
        self.last = [0, 1].map(|ch| f32::from(chunk.samples[(n - 1) * c + ch.min(c - 1)]));
    }
}

pub(super) fn clip(path: &Path, budget: usize) -> Result<Clip, String> {
    let probed = wprs_scene::Clip::file(path)?;
    let mp3 = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("mp3"));
    let seconds = if mp3 { MAX_SOUND_SECONDS } else { probed.seconds.min(MAX_SOUND_SECONDS) };
    let limit = (((seconds + MARGIN) * RATE as f32) as usize).min(budget);
    let (mut decoder, mut resample, deadline) =
        (Decoder::open(path)?, Resample::default(), Instant::now() + WAIT);
    let mut clip = Clip { channels: 0, samples: Vec::new() };
    while let Some(chunk) = decoder.next() {
        clip.channels = chunk.channels.clamp(1, 2);
        resample.run(&chunk, &mut clip.samples);
        if clip.frames() > limit {
            return Err(format!("the sound lasts longer than {:.1} s", limit as f32 / RATE as f32));
        }
        if Instant::now() > deadline {
            return Err(format!("the sound was not decoded within {WAIT:?}"));
        }
    }
    (clip.frames() > 0).then_some(clip).ok_or_else(|| "the file holds no sound".into())
}
