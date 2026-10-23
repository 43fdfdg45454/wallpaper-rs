use std::sync::Arc;
use wprs_scene::Voice;

pub const RATE: u32 = 48_000;
const RAMP: f32 = 1.0 / 960.0;

#[derive(Debug, Clone, PartialEq)]
pub struct Clip {
    pub channels: usize,
    pub samples: Vec<i16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Track {
    pub repeat: bool,
    pub fade_in: f32,
    pub fade_out: f32,
}

#[derive(Default)]
struct Lane {
    track: Track,
    clip: Option<Arc<Clip>>,
    voice: Voice,
    at: usize,
    gain: f32,
    level: f32,
    ended: bool,
}

#[derive(Default)]
pub struct Mix {
    lanes: Vec<Lane>,
    pub frames: u64,
}

impl Clip {
    pub fn frames(&self) -> usize {
        self.samples.len() / self.channels.max(1)
    }
}

impl Mix {
    pub fn new(tracks: &[Track]) -> Mix {
        Mix { lanes: tracks.iter().map(|&track| Lane { track, ..Lane::default() }).collect(), frames: 0 }
    }

    pub fn load(&mut self, lane: usize, clip: Arc<Clip>) {
        if let Some(lane) = self.lanes.get_mut(lane).filter(|_| clip.frames() > 0 && clip.channels <= 2) {
            lane.clip = Some(clip);
        }
    }

    pub fn set(&mut self, voices: &[Voice]) {
        self.lanes.iter_mut().zip(voices).for_each(|(lane, voice)| lane.voice = *voice);
    }

    pub fn playheads(&self) -> Vec<usize> {
        self.lanes.iter().map(|lane| lane.at).collect()
    }

    pub fn fill(&mut self, out: &mut [u8]) {
        for frame in out.as_chunks_mut::<4>().0 {
            let mut sum = [0.0f32; 2];
            self.lanes.iter_mut().for_each(|lane| lane.add(&mut sum));
            let [left, right] =
                sum.map(|s| ((s.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16).to_le_bytes());
            *frame = [left[0], left[1], right[0], right[1]];
        }
        self.frames += (out.len() / 4) as u64;
    }
}

impl Lane {
    fn add(&mut self, sum: &mut [f32; 2]) {
        let Some(clip) = &self.clip else { return };
        let on = self.voice.play && !self.ended;
        if !on && self.gain <= 0.0 {
            (self.at, self.gain, self.ended) = (0, 0.0, self.ended && self.voice.play);
            return;
        }
        let fade = if on { self.track.fade_in } else { self.track.fade_out };
        let step = if fade > 0.0 { 1.0 / (fade * RATE as f32) } else { 1.0 };
        self.gain = if on { (self.gain + step).min(1.0) } else { (self.gain - step).max(0.0) };
        self.level += (self.voice.volume - self.level).clamp(-RAMP, RAMP);
        let (c, pan) = (clip.channels, self.voice.pan);
        let (left, right) = (clip.samples[self.at * c], clip.samples[self.at * c + c - 1]);
        let gain = self.gain * self.level / 32_768.0;
        sum[0] += f32::from(left) * gain * (1.0 - pan).min(1.0);
        sum[1] += f32::from(right) * gain * (1.0 + pan).min(1.0);
        self.at += 1;
        if self.at >= clip.frames() {
            self.at = 0;
            if !self.track.repeat {
                (self.ended, self.gain) = (true, 0.0);
            }
        }
    }
}
