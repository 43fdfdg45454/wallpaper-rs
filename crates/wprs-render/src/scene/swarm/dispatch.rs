use super::stages::{Phase, Task};
use super::step::CUT;
use super::{CHUNK, SIM, Swarms};
use crate::{RenderError, Renderer};
use std::time::Instant;
use wprs_scene::{PARTICLE_EVENTS, Runtime, Sim};

pub(super) fn fresh(alloc: u32, size: u32) -> Vec<u32> {
    let mut state = vec![0u32; size as usize];
    state[0] = alloc;
    state[4..4 + alloc as usize].iter_mut().enumerate().for_each(|(i, v)| *v = i as u32);
    state
}

pub(super) struct Job {
    pub(super) herd: usize,
    pub(super) count: usize,
    pub(super) reset: bool,
}

impl Swarms {
    pub(super) fn dispatch(
        &mut self,
        r: &Renderer,
        runtime: &Runtime,
        jobs: &[Job],
        start: Instant,
    ) -> Result<(), RenderError> {
        if self.compute.is_none() || jobs.is_empty() {
            return Ok(());
        }
        for job in jobs.iter().filter(|j| j.reset) {
            let (bake, Some(flight)) = (&self.herds[job.herd].bake, &self.flights[job.herd]) else {
                continue;
            };
            for (p, store) in flight.stores.iter().enumerate() {
                let state = fresh(bake.allocs[p], bake.states[p]);
                store.gpu.iter().for_each(|g| r.queue.write_buffer(&g.0, 0, bytemuck::cast_slice(&state)));
            }
            let encoder = self.stamp.encoder(r);
            flight.stores.iter().for_each(|s| encoder.clear_buffer(&s.particles, 0, None));
        }
        let longest = jobs.iter().map(|j| j.count).max().unwrap_or(0);
        for first in (0..longest).step_by(CHUNK) {
            let tasks = self.plan(r, runtime, jobs, first);
            let deepest = tasks.iter().map(|t| t.depth).max().unwrap_or(0);
            for k in 0..CHUNK.min(longest - first) {
                for depth in 0..=deepest {
                    self.record(r, &tasks, k, Phase::Simulate(depth));
                    self.record(r, &tasks, k, Phase::Emit(depth));
                }
                self.record(r, &tasks, k, Phase::Settle);
            }
            self.record(r, &tasks, 0, Phase::Twine);
            if first + CHUNK < longest {
                self.stamp.submit(r);
                r.finish()?;
                if start.elapsed() > CUT {
                    break;
                }
            }
        }
        self.sort_all(r, jobs);
        Ok(())
    }

    fn plan(&mut self, r: &Renderer, runtime: &Runtime, jobs: &[Job], first: usize) -> Vec<Task> {
        let mut tasks = Vec::new();
        for job in jobs.iter().filter(|j| j.count > first) {
            let (Some(swarm), bake) = (runtime.swarm(job.herd), &self.herds[job.herd].bake) else { continue };
            let Some(flight) = &self.flights[job.herd] else { continue };
            let steps = first..(first + CHUNK).min(job.count);
            let mut depths: Vec<u32> = Vec::with_capacity(flight.stores.len());
            for (p, store) in flight.stores.iter().enumerate() {
                let depth = bake.hosts[p].and_then(|h| depths.get(h)).map_or(0, |d| d + 1);
                depths.push(depth);
                self.bytes.clear();
                self.bytes.resize(steps.len() * SIM as usize, 0);
                for ((bytes, j), k) in self.bytes.chunks_mut(SIM as usize).zip(steps.clone()).zip(0..) {
                    let sim = bake.apply(p, swarm.sim(j, p));
                    bytes[..size_of::<Sim>()].copy_from_slice(bytemuck::bytes_of(&sim));
                    let threads = match sim.mode[0] {
                        0 => sim.total(),
                        1 | 2 => PARTICLE_EVENTS,
                        _ => sim.mode[3],
                    };
                    let (settle, twine) = (swarm.sim(j, p).mode[1] & 2 != 0, j + 1 == job.count);
                    let (herd, cells) = (job.herd, sim.flock[1]);
                    tasks.push(Task { herd, p, k, depth, cells, threads, settle, twine });
                }
                store.gpu.iter().for_each(|g| r.queue.write_buffer(&g.1, 0, &self.bytes));
            }
        }
        tasks
    }
}
