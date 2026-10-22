use super::{CHUNK, SIM, Swarms};
use crate::{RenderError, Renderer};
use std::time::Instant;
use wprs_scene::{Content, Draw, Frame, Quality, Runtime, Sim};

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
        for i in 0..self.kits.len() {
            let id = self.kits[i].id.as_str();
            let shown =
                |d: &Draw| !d.hidden && matches!(d.content, Content::Particles { system } if system == id);
            let Some(swarm) = runtime.swarm(i).filter(|_| frame.draws.iter().any(shown)) else { continue };
            if self.stores[i].is_none() {
                self.stores[i] = Some(self.store(r, i));
                self.created += 1;
            }
            let mark = std::mem::replace(&mut self.marks[i], (swarm.stamp, swarm.epoch));
            let (kit, Some(store)) = (&self.kits[i], &mut self.stores[i]) else { continue };
            if mark == (swarm.stamp, swarm.epoch) {
                continue;
            }
            let reset = mark.1 != swarm.epoch;
            self.steps += swarm.steps.len() as u32;
            if let Some(cloud) = &mut store.cloud {
                if reset {
                    cloud.clear();
                }
                swarm.steps.iter().for_each(|s| cloud.step(&kit.bake.apply(s), &kit.bake.points));
                r.queue.write_buffer(&store.particles, 0, bytemuck::cast_slice(&cloud.particles));
                continue;
            }
            let (Some((dead, top, sims, group)), Some([simulate, emit])) = (&store.gpu, &self.compute) else {
                continue;
            };
            if reset {
                let free: Vec<u32> = (0..kit.bake.alloc).collect();
                r.queue.write_buffer(dead, 0, bytemuck::cast_slice(&free));
                r.queue.write_buffer(top, 0, bytemuck::bytes_of(&(kit.bake.alloc as i32)));
            }
            for (n, chunk) in swarm.steps.chunks(CHUNK).enumerate() {
                self.bytes.clear();
                self.bytes.resize(chunk.len() * SIM as usize, 0);
                for (bytes, step) in self.bytes.chunks_mut(SIM as usize).zip(chunk) {
                    bytes[..size_of::<Sim>()].copy_from_slice(bytemuck::bytes_of(&kit.bake.apply(step)));
                }
                r.queue.write_buffer(sims, 0, &self.bytes);
                let mut encoder = r.device.create_command_encoder(&Default::default());
                if reset && n == 0 {
                    encoder.clear_buffer(&store.particles, 0, None);
                }
                let mut pass = encoder.begin_compute_pass(&Default::default());
                for (j, step) in chunk.iter().enumerate() {
                    pass.set_bind_group(0, group, &[j as u32 * SIM as u32]);
                    pass.set_pipeline(simulate);
                    pass.dispatch_workgroups(kit.bake.alloc.div_ceil(64), 1, 1);
                    let births = kit.bake.apply(step).births[3];
                    if births > 0 {
                        pass.set_pipeline(emit);
                        pass.dispatch_workgroups(births.div_ceil(64), 1, 1);
                    }
                }
                drop(pass);
                r.queue.submit([encoder.finish()]);
            }
        }
        if self.profile {
            r.finish()?;
        }
        self.micros = start.elapsed().as_micros().min(u128::from(u32::MAX)) as u32;
        Ok(())
    }
}
