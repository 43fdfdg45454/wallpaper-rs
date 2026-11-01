use super::swarm::{Feed, Play};
use super::{Flock, MAX_EMITTERS2, Swarm, System};
use crate::MAX_PARTICLES;
use crate::input::ParticleCommand;

pub const MAX_EMIT: u32 = 1000;

impl Swarm {
    pub(super) fn step(&mut self, systems: &[System], flock: &Flock, dt: f32) {
        for (k, part) in flock.parts.iter().enumerate() {
            let s = &systems[part.system];
            let mut sim = self.frames[k];
            let (first, count) = (self.bases[k].ids[1], self.bases[k].ids[2]);
            (sim.clock[0], sim.clock[1], sim.ids[1], sim.ids[2]) = (dt, self.sim as f32, first, count);
            sim.phases(self.sim);
            self.bases[k].ids[2] = count.wrapping_add(1);
            if sim.mode[0] == 0 {
                let cap = sim.ids[3];
                let mut total = 0u32;
                for (e, em) in s.emitters.iter().take(MAX_EMITTERS2).enumerate() {
                    self.timing(k, e, em);
                    let sim_t = self.sim;
                    let f = &mut self.feeds[k][e];
                    let mut n = 0u32;
                    if f.on {
                        f.owed = (f.owed + f.rate * dt).min(cap as f32);
                        n = f.owed as u32;
                        f.owed -= n as f32;
                        n = n.saturating_add(std::mem::take(&mut f.pending));
                        let every = em.burst.as_ref().and_then(|b| b.every.map(|t| (b.count, f64::from(t))));
                        if let Some((burst, every)) = every
                            && sim_t >= f.next
                        {
                            n = n.saturating_add(burst);
                            let next = f.next + every;
                            f.next = if next > sim_t { next } else { sim_t + every };
                        }
                    } else {
                        f.owed = 0.0;
                    }
                    if e == 0 {
                        n = n.saturating_add(std::mem::take(&mut self.emits));
                    }
                    if let Some(most) = em.max_per_step {
                        let f = &mut self.feeds[k][e];
                        f.pending = f.pending.saturating_add(n.saturating_sub(most)).min(cap);
                        n = n.min(most);
                    }
                    let before = total;
                    total = total.saturating_add(n).min(cap);
                    sim.births[e] = total;
                    let f = &mut self.feeds[k][e];
                    sim.orders[e] = f.sequence;
                    f.sequence = f.sequence.wrapping_add(total - before);
                }
                sim.births[s.emitters.len().min(MAX_EMITTERS2)..].fill(total);
                self.bases[k].ids[1] = first.wrapping_add(total);
            }
            self.outlive(k, &sim, sim.total() > 0);
            self.steps.push(sim);
        }
        self.sim += f64::from(dt);
    }

    pub fn command(&mut self, c: ParticleCommand) {
        match c {
            ParticleCommand::Play => {
                (self.play, self.start) = (Play::Playing, self.sim);
                self.feeds.iter_mut().flatten().for_each(|f| *f = Feed { rate: f.rate, ..Feed::default() });
            }
            ParticleCommand::Pause => self.play = Play::Paused,
            ParticleCommand::Stop => {
                let (clock, sim) = (self.clock, self.sim);
                self.restart();
                (self.clock, self.sim, self.start, self.play) = (clock, sim, sim, Play::Stopped);
            }
            ParticleCommand::Emit(n) => {
                self.emits = self.emits.saturating_add(n.min(MAX_EMIT)).min(MAX_PARTICLES)
            }
        }
    }
}
