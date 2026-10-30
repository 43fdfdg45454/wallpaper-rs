use super::Swarms;
use crate::{RenderError, Renderer};
use std::time::{Duration, Instant};
use wprs_scene::{Cloud, Content, Draw, Frame, Quality, Runtime, Sim};

pub(super) const CUT: Duration = Duration::from_millis(200);

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
            self.dispatch(r, i, &swarm, (count, reset), start)?;
        }
        if self.profile {
            r.finish()?;
        }
        self.micros = start.elapsed().as_micros().min(u128::from(u32::MAX)) as u32;
        Ok(())
    }
}
