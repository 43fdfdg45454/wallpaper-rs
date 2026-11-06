use super::pipes::Shader;
use super::{Op, Src};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

const EVERY: u64 = 15;
const MOST: u32 = 256;
const WAITING: usize = 3;
const KEPT: usize = 1_024;

static TURN: AtomicU64 = AtomicU64::new(0);
static WAITS: Mutex<Vec<Waiting>> = Mutex::new(Vec::new());
type Sample = (Instant, Vec<(String, f64)>);

static SAMPLES: Mutex<VecDeque<Sample>> = Mutex::new(VecDeque::new());

struct Waiting {
    read: wgpu::Buffer,
    labels: Vec<String>,
    period: f64,
    ready: Arc<AtomicBool>,
    asked: bool,
}

pub(crate) struct Profile {
    set: wgpu::QuerySet,
    labels: Vec<String>,
    open: bool,
}

pub fn reset_profiled_passes() {
    WAITS.lock().unwrap_or_else(PoisonError::into_inner).clear();
    SAMPLES.lock().unwrap_or_else(PoisonError::into_inner).clear();
}

pub fn profiled_passes(within: Option<Duration>) -> (u32, Vec<(String, f64)>) {
    gather();
    let samples = SAMPLES.lock().unwrap_or_else(PoisonError::into_inner);
    let recent = |s: &&Sample| within.is_none_or(|w| s.0.elapsed() <= w);
    let mut sums: Vec<(String, f64)> = Vec::new();
    let mut frames = 0;
    for (_, passes) in samples.iter().filter(recent) {
        frames += 1;
        for (label, ms) in passes {
            match sums.iter_mut().find(|(l, _)| l == label) {
                Some(found) => found.1 += ms,
                None => sums.push((label.clone(), *ms)),
            }
        }
    }
    let mean = |(label, ms): (String, f64)| (label, ms / f64::from(frames.max(1)));
    (frames, sums.into_iter().map(mean).collect())
}

pub(crate) fn label(op: &Op, size: (u32, u32), format: wgpu::TextureFormat) -> String {
    let kind = match op {
        Op::Copy { .. } => "copy".to_owned(),
        Op::Effect(f) => match f.shader {
            Shader::Scene(entry) | Shader::Lit(entry) | Shader::Fx(_, entry) => format!("effect {entry}"),
            Shader::Own(..) => "package effect".to_owned(),
        },
        Op::Particles { .. } => "particles".to_owned(),
        Op::Clear(..) => "clear and draw".to_owned(),
        Op::Draw { entry, .. } | Op::Lit { entry, .. } => format!("draw {entry}"),
        Op::Strip { .. } => "strip".to_owned(),
        Op::Read { .. } => "read".to_owned(),
    };
    format!("{kind} {}x{} {format:?}", size.0, size.1)
}

pub(crate) fn target(op: &Op) -> Option<Src> {
    match op {
        Op::Copy { to, .. } => Some(*to),
        Op::Effect(f) => Some(f.target),
        Op::Particles { target, .. } | Op::Draw { target, .. } | Op::Lit { target, .. } => Some(*target),
        Op::Clear(target, _) => Some(*target),
        Op::Strip { .. } | Op::Read { .. } => None,
    }
}

impl Profile {
    pub(crate) fn begin(device: &wgpu::Device, on: bool) -> Option<Profile> {
        let inside = wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS;
        if !on || !device.features().contains(inside) {
            return None;
        }
        gather();
        let busy = WAITS.lock().unwrap_or_else(PoisonError::into_inner).len() >= WAITING;
        if busy || !TURN.fetch_add(1, Relaxed).is_multiple_of(EVERY) {
            return None;
        }
        let set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("wprs-scene passes"),
            ty: wgpu::QueryType::Timestamp,
            count: MOST,
        });
        Some(Profile { set, labels: Vec::new(), open: false })
    }

    pub(crate) fn start(&mut self, encoder: &mut wgpu::CommandEncoder, label: impl FnOnce() -> String) {
        let at = self.labels.len() as u32 * 2;
        self.open = at + 2 <= MOST;
        if self.open {
            encoder.write_timestamp(&self.set, at);
            self.labels.push(label());
        }
    }

    pub(crate) fn end(&mut self, encoder: &mut wgpu::CommandEncoder) {
        if std::mem::take(&mut self.open) {
            encoder.write_timestamp(&self.set, self.labels.len() as u32 * 2 - 1);
        }
    }

    pub(crate) fn finish(
        self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let count = self.labels.len() as u32 * 2;
        if count == 0 {
            return;
        }
        let size = u64::from(count) * 8;
        let buffer = |usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size,
                usage,
                mapped_at_creation: false,
            })
        };
        let resolved = buffer(wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC);
        let read = buffer(wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ);
        encoder.resolve_query_set(&self.set, 0..count, &resolved, 0);
        encoder.copy_buffer_to_buffer(&resolved, 0, &read, 0, size);
        let period = f64::from(queue.get_timestamp_period());
        let ready = Arc::new(AtomicBool::new(false));
        let waiting = Waiting { read, labels: self.labels, period, ready, asked: false };
        WAITS.lock().unwrap_or_else(PoisonError::into_inner).push(waiting);
    }
}

pub(crate) fn submitted() {
    let mut waits = WAITS.lock().unwrap_or_else(PoisonError::into_inner);
    for w in waits.iter_mut().filter(|w| !w.asked) {
        let ready = w.ready.clone();
        w.read.slice(..).map_async(wgpu::MapMode::Read, move |_| ready.store(true, Relaxed));
        w.asked = true;
    }
}

fn gather() {
    let mut waits = WAITS.lock().unwrap_or_else(PoisonError::into_inner);
    let (done, left): (Vec<_>, Vec<_>) = waits.drain(..).partition(|w| w.ready.load(Relaxed));
    *waits = left;
    let mut samples = SAMPLES.lock().unwrap_or_else(PoisonError::into_inner);
    for w in done {
        let ticks: Vec<u64> = match w.read.slice(..).get_mapped_range() {
            Ok(data) => data.as_chunks::<8>().0.iter().map(|b| u64::from_le_bytes(*b)).collect(),
            Err(_) => continue,
        };
        w.read.unmap();
        let ms = |i: usize| ticks[2 * i + 1].saturating_sub(ticks[2 * i]) as f64 * w.period / 1e6;
        let passes = w.labels.into_iter().enumerate().map(|(i, l)| (l, ms(i))).collect();
        samples.push_back((Instant::now(), passes));
        while samples.len() > KEPT {
            samples.pop_front();
        }
    }
}
