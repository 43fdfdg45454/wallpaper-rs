use std::path::PathBuf;
use std::sync::Arc;
use wprs_scene::{Playlist, Voice};

mod lane;

use lane::Lane;

pub const RATE: u32 = 48_000;
const RAMP: f32 = 1.0 / 960.0;
const DUCK: f32 = 1.0 / 240.0;
const DRIFT: f64 = 0.06 * RATE as f64;
const AHEAD: f64 = 1.5 * RATE as f64;

#[derive(Debug, Clone, PartialEq)]
pub struct Clip {
    pub channels: usize,
    pub samples: Vec<i16>,
}

#[derive(Debug, Clone)]
pub enum Source {
    Memory(Arc<Clip>),
    Stream(Arc<PathBuf>),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Track {
    pub list: Playlist,
    pub fade_in: f32,
    pub fade_out: f32,
}

type Item = (Option<usize>, f64, f64);

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
        let lane = |track: &Track| Lane {
            sources: vec![None; track.list.lengths.len()],
            track: track.clone(),
            duck: 1.0,
            ..Lane::default()
        };
        Mix { lanes: tracks.iter().map(lane).collect(), frames: 0 }
    }

    pub fn load(&mut self, lane: usize, clip: usize, source: Source) {
        let fits = |s: &Source| !matches!(s, Source::Memory(c) if c.frames() == 0 || c.channels > 2);
        if let Some(slot) =
            self.lanes.get_mut(lane).and_then(|l| l.sources.get_mut(clip)).filter(|_| fits(&source))
        {
            *slot = Some(source);
        }
    }

    pub fn set(&mut self, voices: &[Voice], heard: f64) {
        for (lane, voice) in self.lanes.iter_mut().zip(voices) {
            (lane.voice, lane.aim) = (*voice, voice.at * f64::from(RATE) - heard);
        }
    }

    pub fn playheads(&self) -> Vec<f64> {
        self.lanes.iter().map(|lane| lane.at).collect()
    }

    pub fn quiet(&self) -> bool {
        self.lanes.iter().all(|l| !l.voice.play || !l.sounding())
    }

    pub fn fill(&mut self, out: &mut [u8]) {
        let now = self.frames as f64;
        self.lanes.iter_mut().for_each(|lane| lane.prepare(now));
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

fn same(a: f64, b: f64) -> bool {
    (a - b).abs() < 2.0
}
