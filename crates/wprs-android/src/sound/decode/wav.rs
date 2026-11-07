use super::Chunk;
use std::path::Path;
use wprs_mix::RATE;

const FRAMES: usize = 4800;

pub(crate) struct Decoder {
    rate: u32,
    channels: usize,
    samples: Vec<i16>,
    at: usize,
}

fn field(bytes: &[u8], at: usize, len: usize) -> usize {
    bytes.get(at..at + len).map_or(0, |b| b.iter().rev().fold(0, |v, &x| v << 8 | usize::from(x)))
}

impl Decoder {
    pub(crate) fn open(path: &Path) -> Result<Decoder, String> {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let (mut rate, mut channels, mut at, mut samples) = (RATE, 1, 12, Vec::new());
        while at + 8 <= bytes.len() {
            let (id, size) = (&bytes[at..at + 4], field(&bytes, at + 4, 4));
            let body = at + 8;
            match id {
                b"fmt " => (channels, rate) = (field(&bytes, body + 2, 2), field(&bytes, body + 4, 4) as u32),
                b"data" => {
                    let end = (body + size).min(bytes.len());
                    samples =
                        bytes[body..end].as_chunks::<2>().0.iter().map(|b| i16::from_le_bytes(*b)).collect();
                }
                _ => {}
            }
            at = body + size + (size & 1);
        }
        (bytes.starts_with(b"RIFF") && !samples.is_empty())
            .then_some(Decoder { rate, channels, samples, at: 0 })
            .ok_or_else(|| "is not a PCM WAV sound".into())
    }

    pub(crate) fn seek(&mut self, us: i64) {
        self.at = (us.max(0) as u64 * u64::from(self.rate) / 1_000_000) as usize * self.channels;
    }

    pub(crate) fn next(&mut self) -> Option<Chunk> {
        let end = (self.at + FRAMES * self.channels).min(self.samples.len());
        let start_us = (self.at / self.channels) as i64 * 1_000_000 / i64::from(self.rate);
        let samples = self.samples.get(self.at..end).filter(|s| !s.is_empty())?.to_vec();
        self.at = end;
        Some(Chunk { rate: self.rate, channels: self.channels, start_us, samples })
    }
}
