use super::plan::SLOTS;
use crate::Renderer;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering::Relaxed};

const BACKS: usize = 4;
const BYTES: u64 = SLOTS as u64 * 16 + 16;
pub(crate) const SIM_AT: u32 = SLOTS as u32 * 2;
const FREE: u8 = 0;
const BUSY: u8 = 1;
const READY: u8 = 2;
const FAILED: u8 = 3;

fn ticks(t: &[u8; 16]) -> (u64, u64) {
    let tick = |b: &[u8]| u64::from_le_bytes(b.try_into().unwrap_or_default());
    (tick(&t[..8]), tick(&t[8..]))
}

pub(super) fn busy(spans: [(u64, u64); 2], edge: u64) -> (u64, u64) {
    let [a, b] = spans.map(|(s, e)| (s.max(edge), e.max(s).max(edge)));
    let [a, b] = if a.0 <= b.0 { [a, b] } else { [b, a] };
    (a.1 - a.0 + b.1.saturating_sub(b.0.max(a.1)), a.1.max(b.1))
}

struct Back {
    buffer: wgpu::Buffer,
    state: Arc<AtomicU8>,
    runs: Vec<(u64, bool)>,
    wide: bool,
    sim: bool,
    serial: u64,
}

pub(super) struct Stamps {
    pub(super) set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    backs: Vec<Back>,
    pub(super) period: f64,
    sent: u64,
}

impl Stamps {
    pub(super) fn new(r: &Renderer) -> Stamps {
        let (label, size, mapped_at_creation) = (Some("wprs-scene clock"), BYTES, false);
        let (ty, count) = (wgpu::QueryType::Timestamp, SIM_AT + 2);
        let set = r.device.create_query_set(&wgpu::QuerySetDescriptor { label, ty, count });
        let buffer = |usage| {
            r.device.create_buffer(&wgpu::BufferDescriptor { label, size, usage, mapped_at_creation })
        };
        let resolve = buffer(wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC);
        let backs = (0..BACKS)
            .map(|_| Back {
                buffer: buffer(wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ),
                state: Arc::default(),
                runs: Vec::new(),
                wide: false,
                sim: false,
                serial: 0,
            })
            .collect();
        Stamps { set, resolve, backs, period: f64::from(r.queue.get_timestamp_period()), sent: 0 }
    }

    pub(super) fn free(&self) -> Option<usize> {
        self.backs.iter().position(|b| b.state.load(Relaxed) == FREE)
    }

    pub(super) fn take(&mut self, back: usize, runs: &mut Vec<(u64, bool)>, (wide, sim): (bool, bool)) {
        std::mem::swap(&mut self.backs[back].runs, runs);
        self.sent += 1;
        let b = &mut self.backs[back];
        (b.wide, b.sim, b.serial) = (wide, sim, self.sent);
    }

    pub(super) fn mark(&self, encoder: &mut wgpu::CommandEncoder, end: bool) {
        let (begin, finish) = if end { (None, Some(1)) } else { (Some(0), None) };
        let timestamp_writes = Some(wgpu::ComputePassTimestampWrites {
            query_set: &self.set,
            beginning_of_pass_write_index: begin,
            end_of_pass_write_index: finish,
        });
        drop(encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None, timestamp_writes }));
    }

    pub(super) fn close(&self, encoder: &mut wgpu::CommandEncoder, back: usize) {
        self.mark(encoder, true);
        let Back { buffer, state, runs, sim, .. } = &self.backs[back];
        let queries = (runs.len() as u32 + 1) * 2;
        encoder.resolve_query_set(&self.set, 0..queries, &self.resolve, 0);
        encoder.copy_buffer_to_buffer(&self.resolve, 0, buffer, 0, u64::from(queries) * 8);
        if *sim {
            let at = u64::from(SIM_AT) * 8;
            encoder.resolve_query_set(&self.set, SIM_AT..SIM_AT + 2, &self.resolve, at);
            encoder.copy_buffer_to_buffer(&self.resolve, at, buffer, at, 16);
        }
        state.store(BUSY, Relaxed);
        let flag = state.clone();
        encoder.map_buffer_on_submit(buffer, wgpu::MapMode::Read, .., move |done| {
            flag.store(if done.is_ok() { READY } else { FAILED }, Relaxed);
        });
    }

    pub(super) fn collect(
        &mut self,
        out: &mut Vec<(u64, bool, f64)>,
        mut each: impl FnMut(&[(u64, bool, f64)], [(u64, u64); 2], bool),
    ) {
        let (period, mut done, mut n) = (self.period, [0; BACKS], 0);
        for (k, back) in self.backs.iter().enumerate() {
            if matches!(back.state.load(Relaxed), READY | FAILED) {
                (done[n], n) = (k, n + 1);
            }
        }
        done[..n].sort_unstable_by_key(|k| self.backs[*k].serial);
        for &k in &done[..n] {
            let back = &mut self.backs[k];
            if back.state.load(Relaxed) == READY {
                if let Ok(data) = back.buffer.slice(..).get_mapped_range() {
                    let times = data.as_chunks::<16>().0;
                    let ms = |t: &[u8; 16]| {
                        let (s, e) = ticks(t);
                        e.saturating_sub(s) as f64 * period / 1e6
                    };
                    out.clear();
                    let runs = back.runs.iter().zip(times.iter().skip(1));
                    out.extend(runs.map(|(&(key, own), t)| (key, own, ms(t))));
                    let at = |k: usize| times.get(k).map_or((0, 0), ticks);
                    each(out, [at(0), if back.sim { at(SLOTS) } else { (0, 0) }], back.wide);
                }
                back.buffer.unmap();
            }
            back.runs.clear();
            back.state.store(FREE, Relaxed);
        }
    }
}
