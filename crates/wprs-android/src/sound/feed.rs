use super::decode::{Decoder, Resample};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use wprs_mix::{Feed, RATE, Stream};

const NEAR: u64 = RATE as u64 / 4;
const AHEAD: u64 = RATE as u64 * 2;

pub(super) static BLOCKS: AtomicU32 = AtomicU32::new(0);

#[derive(Debug)]
pub(super) struct Disk(pub(super) PathBuf);

struct Block {
    first: u64,
    channels: usize,
    samples: Vec<i16>,
}

#[derive(Default)]
struct Ring {
    blocks: VecDeque<Block>,
    end: u64,
    seek: Option<u64>,
}

#[derive(Default)]
struct Shared {
    ring: Mutex<Ring>,
    signal: Condvar,
    quit: AtomicBool,
}

struct Streamed {
    shared: Arc<Shared>,
    current: Option<Block>,
}

impl Block {
    fn len(&self) -> u64 {
        (self.samples.len() / self.channels) as u64
    }

    fn at(&self, at: u64) -> Option<[i16; 2]> {
        let i =
            usize::try_from(at.checked_sub(self.first).filter(|d| *d < self.len())?).ok()? * self.channels;
        Some([self.samples[i], self.samples[i + self.channels - 1]])
    }
}

fn locked(ring: &Mutex<Ring>) -> MutexGuard<'_, Ring> {
    ring.lock().unwrap_or_else(PoisonError::into_inner)
}

fn micros(frame: u64) -> i64 {
    (frame * 1_000_000 / u64::from(RATE)) as i64
}

fn work(shared: &Shared, path: &std::path::Path, from: u64) {
    let Ok(mut decoder) = Decoder::open(path) else { return };
    let (mut resample, mut next, mut first, mut done) = (Resample::default(), from, true, false);
    decoder.seek(micros(from));
    loop {
        let mut ring = locked(&shared.ring);
        while !shared.quit.load(Relaxed) {
            if let Some(to) = ring.seek.take() {
                (ring.end, next, first, done, resample) = (to, to, true, false, Resample::default());
                ring.blocks.clear();
                decoder.seek(micros(to));
            }
            let held = ring.blocks.front().map_or(0, |b| ring.end - b.first);
            if !done && held < AHEAD {
                break;
            }
            ring = shared.signal.wait(ring).unwrap_or_else(PoisonError::into_inner);
        }
        drop(ring);
        if shared.quit.load(Relaxed) {
            return;
        }
        let Some(chunk) = decoder.next() else {
            done = true;
            continue;
        };
        let mut samples = Vec::new();
        resample.run(&chunk, &mut samples);
        if std::mem::take(&mut first) {
            next = micros_to_frames(chunk.start_us);
        }
        let block = Block { first: next, channels: chunk.channels.clamp(1, 2), samples };
        next += block.len();
        let mut ring = locked(&shared.ring);
        if ring.seek.is_none() {
            ring.end = next;
            ring.blocks.push_back(block);
        }
    }
}

fn micros_to_frames(us: i64) -> u64 {
    us.max(0) as u64 * u64::from(RATE) / 1_000_000
}

impl Stream for Disk {
    fn open(&self, from: u64) -> Box<dyn Feed> {
        let shared = Arc::new(Shared::default());
        let (worker, path) = (shared.clone(), self.0.clone());
        let spawned = std::thread::Builder::new()
            .name("wprs-sound-feed".into())
            .spawn(move || work(&worker, &path, from));
        if let Err(e) = spawned {
            log::warn!("cannot start streaming a sound: {e}");
        }
        Box::new(Streamed { shared, current: None })
    }
}

impl Streamed {
    fn refill(&mut self, at: u64) {
        let Ok(mut ring) = self.shared.ring.try_lock() else { return };
        while ring.blocks.front().is_some_and(|b| b.first + b.len() <= at) {
            ring.blocks.pop_front();
        }
        let behind = ring.blocks.front().is_some_and(|b| at + NEAR < b.first);
        let ahead = at >= ring.end + NEAR;
        if (behind || ahead) && ring.seek.is_none() {
            ring.seek = Some(at);
            ring.blocks.clear();
        } else if ring.blocks.front().is_some_and(|b| b.first <= at) {
            self.current = ring.blocks.pop_front();
            BLOCKS.fetch_add(1, Relaxed);
        }
        self.shared.signal.notify_one();
    }
}

impl Feed for Streamed {
    fn settle(&mut self) {}

    fn frame(&mut self, at: u64) -> Option<[i16; 2]> {
        if let Some(sample) = self.current.as_ref().and_then(|b| b.at(at)) {
            return Some(sample);
        }
        self.refill(at);
        self.current.as_ref()?.at(at)
    }
}

impl Drop for Streamed {
    fn drop(&mut self) {
        self.shared.quit.store(true, Relaxed);
        self.shared.signal.notify_all();
    }
}
