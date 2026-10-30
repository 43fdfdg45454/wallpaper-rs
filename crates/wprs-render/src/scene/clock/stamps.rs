use super::plan::SLOTS;
use crate::Renderer;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering::Relaxed};

const BACKS: usize = 4;
const BYTES: u64 = SLOTS as u64 * 16;
const FREE: u8 = 0;
const BUSY: u8 = 1;
const READY: u8 = 2;
const FAILED: u8 = 3;

fn ms(begin: &[u8], end: &[u8], period: f64) -> f64 {
    let tick = |b: &[u8]| u64::from_le_bytes(b.try_into().unwrap_or_default());
    tick(end).saturating_sub(tick(begin)) as f64 * period / 1e6
}

struct Back {
    buffer: wgpu::Buffer,
    state: Arc<AtomicU8>,
    runs: Vec<u64>,
    whole: bool,
}

pub(super) struct Stamps {
    pub(super) set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    backs: Vec<Back>,
    period: f64,
}

impl Stamps {
    pub(super) fn new(r: &Renderer) -> Stamps {
        let (label, size, mapped_at_creation) = (Some("wprs-scene clock"), BYTES, false);
        let (ty, count) = (wgpu::QueryType::Timestamp, SLOTS as u32 * 2);
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
                whole: false,
            })
            .collect();
        Stamps { set, resolve, backs, period: f64::from(r.queue.get_timestamp_period()) }
    }

    pub(super) fn free(&self) -> Option<usize> {
        self.backs.iter().position(|b| b.state.load(Relaxed) == FREE)
    }

    pub(super) fn take(&mut self, back: usize, runs: &mut Vec<u64>, whole: bool) {
        std::mem::swap(&mut self.backs[back].runs, runs);
        self.backs[back].whole = whole;
    }

    pub(super) fn close(&self, encoder: &mut wgpu::CommandEncoder, back: usize) {
        let Back { buffer, state, runs, .. } = &self.backs[back];
        let queries = runs.len() as u32 * 2;
        encoder.resolve_query_set(&self.set, 0..queries, &self.resolve, 0);
        encoder.copy_buffer_to_buffer(&self.resolve, 0, buffer, 0, u64::from(queries) * 8);
        state.store(BUSY, Relaxed);
        let flag = state.clone();
        encoder.map_buffer_on_submit(buffer, wgpu::MapMode::Read, .., move |done| {
            flag.store(if done.is_ok() { READY } else { FAILED }, Relaxed);
        });
    }

    pub(super) fn collect(
        &mut self,
        out: &mut Vec<(u64, f64)>,
        mut each: impl FnMut(&[(u64, f64)], Option<f64>),
    ) {
        let period = self.period;
        for back in &mut self.backs {
            match back.state.load(Relaxed) {
                READY => {
                    if let Ok(data) = back.buffer.slice(..).get_mapped_range() {
                        let times = data.as_chunks::<16>().0.iter().map(|t| ms(&t[..8], &t[8..], period));
                        out.clear();
                        out.extend(back.runs.iter().copied().zip(times));
                        let end = data.get((out.len() * 16).saturating_sub(8)..out.len() * 16);
                        each(out, end.filter(|_| back.whole).map(|end| ms(&data[..8], end, period)));
                    }
                    back.buffer.unmap();
                }
                FAILED => {}
                _ => continue,
            }
            back.runs.clear();
            back.state.store(FREE, Relaxed);
        }
    }
}
