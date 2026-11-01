use super::step::CUT;
use super::{CHUNK, SIM, Swarms};
use crate::{RenderError, Renderer};
use std::time::Instant;
use wprs_scene::{PARTICLE_EVENTS, Sim, Swarm};

pub(super) fn fresh(alloc: u32, size: u32) -> Vec<u32> {
    let mut state = vec![0u32; size as usize];
    state[0] = alloc;
    state[4..4 + alloc as usize].iter_mut().enumerate().for_each(|(i, v)| *v = i as u32);
    state
}

impl Swarms {
    pub(super) fn dispatch(
        &mut self,
        r: &Renderer,
        i: usize,
        swarm: &Swarm,
        (count, reset): (usize, bool),
        start: Instant,
    ) -> Result<(), RenderError> {
        let (bake, Some(flight)) = (&self.herds[i].bake, &mut self.flights[i]) else { return Ok(()) };
        let Some([simulate, emit, settle]) = &self.compute else { return Ok(()) };
        if reset {
            for (p, store) in flight.stores.iter().enumerate() {
                let state = fresh(bake.allocs[p], bake.states[p]);
                store.gpu.iter().for_each(|g| r.queue.write_buffer(&g.0, 0, bytemuck::cast_slice(&state)));
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
            super::step::mark(&mut self.stamp, &mut encoder, false);
            if reset && n == 0 {
                flight.stores.iter().for_each(|s| encoder.clear_buffer(&s.particles, 0, None));
            }
            let mut pass = encoder.begin_compute_pass(&Default::default());
            for (k, j) in steps.enumerate() {
                let offset = [k as u32 * SIM as u32];
                for (p, store) in flight.stores.iter().enumerate() {
                    let Some((_, _, group)) = &store.gpu else { continue };
                    let sim = bake.apply(p, swarm.sim(j, p));
                    if let (Some(flock), Some([clear, tally, scan, scatter, steer])) =
                        (&store.flock, &self.flocking)
                    {
                        pass.set_bind_group(0, flock, &offset);
                        let each = bake.allocs[p].div_ceil(64);
                        for (pipe, n) in [
                            (clear, sim.flock[1].div_ceil(64)),
                            (tally, each),
                            (scan, 1),
                            (scatter, each),
                            (steer, each),
                        ] {
                            pass.set_pipeline(pipe);
                            pass.dispatch_workgroups(n, 1, 1);
                        }
                    }
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
                    let Some((_, _, group)) = store.gpu.as_ref().filter(|_| swarm.sim(j, p).mode[1] & 2 != 0)
                    else {
                        continue;
                    };
                    pass.set_bind_group(0, group, &offset);
                    pass.set_pipeline(settle);
                    pass.dispatch_workgroups(1, 1, 1);
                }
            }
            if first + CHUNK >= count
                && let Some(twine) = &self.twining
            {
                let offset = [((count - 1 - first) * SIM as usize) as u32];
                for store in flight.stores.iter().filter(|s| s.rope.is_some()) {
                    let Some(group) = &store.twine else { continue };
                    pass.set_bind_group(0, group, &offset);
                    pass.set_pipeline(twine);
                    pass.dispatch_workgroups(1, 1, 1);
                }
            }
            drop(pass);
            super::step::mark(&mut self.stamp, &mut encoder, true);
            r.queue.submit([encoder.finish()]);
            if first + CHUNK < count {
                r.finish()?;
                if start.elapsed() > CUT {
                    break;
                }
            }
        }
        let sorts = (self.sorts.as_ref(), self.canvas[1]);
        super::sort::sort(r, sorts, &mut flight.stores, (&[], &bake.allocs), &mut self.stamp);
        Ok(())
    }
}
