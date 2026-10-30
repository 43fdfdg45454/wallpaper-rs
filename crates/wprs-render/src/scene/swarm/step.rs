use super::store::fresh;
use super::{CHUNK, SIM, Swarms};
use crate::{RenderError, Renderer};
use std::time::{Duration, Instant};
use wprs_scene::{Cloud, Content, Draw, Frame, PARTICLE_EVENTS, Quality, Runtime, Sim};

const CUT: Duration = Duration::from_millis(200);

impl Swarms {
    pub(crate) fn run(
        &mut self,
        r: &Renderer,
        runtime: &Runtime,
        frame: &Frame<'_>,
        quality: Quality,
    ) -> Result<(), RenderError> {
        let start = Instant::now();
        (self.steps, self.created, self.quality) = (0, 0, quality);
        self.looks.clear();
        self.kitted.clear();
        for i in 0..self.herds.len() {
            let shown =
                |d: &Draw| !d.hidden && matches!(d.content, Content::Particles { flock, .. } if flock == i);
            let Some(swarm) = runtime.swarm(i).filter(|_| frame.draws.iter().any(shown)) else { continue };
            if self.flights[i].is_none() {
                self.flights[i] = Some(self.flight(r, i));
                self.created += 1;
            }
            let mark = std::mem::replace(&mut self.marks[i], (swarm.stamp, swarm.epoch));
            let (bake, Some(flight)) = (&self.herds[i].bake, &mut self.flights[i]) else { continue };
            if mark == (swarm.stamp, swarm.epoch) {
                continue;
            }
            let (reset, parts, count) = (mark.1 != swarm.epoch, bake.allocs.len(), swarm.count());
            self.steps += count as u32;
            if !flight.clouds.is_empty() {
                if reset {
                    flight.clouds.iter_mut().for_each(Cloud::clear);
                }
                flight.sims.resize(parts, Sim::default());
                for j in 0..count {
                    if start.elapsed() > CUT {
                        break;
                    }
                    (0..parts).for_each(|p| flight.sims[p] = bake.apply(p, swarm.sim(j, p)));
                    Cloud::brood(&mut flight.clouds, &flight.sims, &bake.hosts, &bake.points);
                }
                for (store, cloud) in flight.stores.iter().zip(&flight.clouds) {
                    r.queue.write_buffer(&store.particles, 0, bytemuck::cast_slice(&cloud.particles));
                    if let Some(trail) = store.trail.as_ref().filter(|_| !cloud.trail.is_empty()) {
                        r.queue.write_buffer(trail, 0, bytemuck::cast_slice(&cloud.trail));
                    }
                }
                super::sort::sort(
                    r,
                    (self.sorts.as_ref(), self.canvas[1]),
                    &mut flight.stores,
                    &flight.clouds,
                    &bake.allocs,
                );
                continue;
            }
            let Some([simulate, emit, settle]) = &self.compute else { continue };
            if reset {
                for (p, store) in flight.stores.iter().enumerate() {
                    let state = fresh(bake.allocs[p], bake.states[p]);
                    store
                        .gpu
                        .iter()
                        .for_each(|g| r.queue.write_buffer(&g.0, 0, bytemuck::cast_slice(&state)));
                }
            }
            for (n, first) in (0..count).step_by(CHUNK).enumerate() {
                let steps = first..(first + CHUNK).min(count);
                for (p, store) in flight.stores.iter().enumerate() {
                    self.bytes.clear();
                    self.bytes.resize(steps.len() * SIM as usize, 0);
                    for (bytes, j) in self.bytes.chunks_mut(SIM as usize).zip(steps.clone()) {
                        bytes[..size_of::<Sim>()]
                            .copy_from_slice(bytemuck::bytes_of(&bake.apply(p, swarm.sim(j, p))));
                    }
                    store.gpu.iter().for_each(|g| r.queue.write_buffer(&g.1, 0, &self.bytes));
                }
                let mut encoder = r.device.create_command_encoder(&Default::default());
                if reset && n == 0 {
                    flight.stores.iter().for_each(|s| encoder.clear_buffer(&s.particles, 0, None));
                }
                let mut pass = encoder.begin_compute_pass(&Default::default());
                for (k, j) in steps.enumerate() {
                    let offset = [k as u32 * SIM as u32];
                    for (p, store) in flight.stores.iter().enumerate() {
                        let Some((_, _, group)) = &store.gpu else { continue };
                        let sim = bake.apply(p, swarm.sim(j, p));
                        pass.set_bind_group(0, group, &offset);
                        pass.set_pipeline(simulate);
                        pass.dispatch_workgroups(bake.allocs[p].div_ceil(64), 1, 1);
                        let threads = match sim.mode[0] {
                            0 => sim.total(),
                            1 | 2 => PARTICLE_EVENTS,
                            _ => sim.mode[3],
                        };
                        if threads > 0 {
                            pass.set_pipeline(emit);
                            pass.dispatch_workgroups(threads.div_ceil(64), 1, 1);
                        }
                    }
                    for (p, store) in flight.stores.iter().enumerate() {
                        let Some((_, _, group)) =
                            store.gpu.as_ref().filter(|_| swarm.sim(j, p).mode[1] & 2 != 0)
                        else {
                            continue;
                        };
                        pass.set_bind_group(0, group, &offset);
                        pass.set_pipeline(settle);
                        pass.dispatch_workgroups(1, 1, 1);
                    }
                }
                drop(pass);
                r.queue.submit([encoder.finish()]);
                if first + CHUNK < count {
                    r.finish()?;
                    if start.elapsed() > CUT {
                        break;
                    }
                }
            }
            super::sort::sort(
                r,
                (self.sorts.as_ref(), self.canvas[1]),
                &mut flight.stores,
                &[],
                &bake.allocs,
            );
        }
        if self.profile {
            r.finish()?;
        }
        self.micros = start.elapsed().as_micros().min(u128::from(u32::MAX)) as u32;
        Ok(())
    }
}
