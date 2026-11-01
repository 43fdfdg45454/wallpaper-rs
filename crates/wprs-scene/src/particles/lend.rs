use super::budget::MAX_ROPE;
use super::{Bake, MAX_COLLIDERS2, MAX_EMITTERS2, Sim};
use crate::{Quality, Tier};

pub(super) fn quality(code: u32) -> Quality {
    match code {
        1 => Quality::Q0,
        2 => Quality::Q1,
        3 => Quality::Q2,
        _ => Quality::Q3,
    }
}

impl Bake {
    pub fn apply(&self, part: usize, sim: &Sim) -> Sim {
        let mut out = *sim;
        let alloc = self.allocs[part];
        out.counts[0] = alloc;
        out.ids[3] = sim.ids[3].min(alloc);
        out.mode[2] = sim.mode[2].min(out.ids[3]);
        out.links = self.links[part];
        out.mode[3] = self.hosts[part].map_or(0, |h| self.allocs[h]);
        out.trail = [self.marks[part], self.trails[part], self.strides[part], u32::from(self.longs[part])];
        for k in 0..MAX_COLLIDERS2 {
            match out.colliders[4 * k][0] as u32 {
                4 => out.colliders[4 * k] = self.fields[part][k],
                7 => self.walls[part][k].fit(&mut out.colliders[4 * k..4 * k + 3]),
                _ => {}
            }
        }
        let (tier, q) = (self.tier.unwrap_or(Tier::T3), quality(sim.flock[2]));
        let [grid, cells, steer] = self.grids[part];
        out.flock = match cells {
            0 => [0; 4],
            _ => [grid, cells, q.neighbours().min(tier.neighbours()), steer],
        };
        out.rope = match self.ropes[part].filter(|_| sim.mode[0] == 0) {
            Some(r) => {
                let shape = q.subdivide(r.subdivide.min(7)) | u32::from(r.smooth) << 8;
                [alloc.min(MAX_ROPE), self.chains[part], shape, tier.rope()]
            }
            None => [0; 4],
        };
        if sim.mode[0] != 0 {
            return out;
        }
        let (mut total, mut last) = (0u32, 0);
        for e in 0..MAX_EMITTERS2 {
            let n = sim.births[e].saturating_sub(last);
            last = sim.births[e];
            let mask = out.shapes[e] == 5;
            if mask {
                out.emitters[4 * e] = self.ranges[part][e];
            }
            let [start, count] = self.layers[part][e];
            if out.shapes[e] == 8 {
                (out.emitters[4 * e][0], out.emitters[4 * e][1]) = (start as f32, count as f32);
            }
            let empty = (mask && self.ranges[part][e][1] < 1.0) || (out.shapes[e] == 8 && count == 0);
            total = total.saturating_add(if empty { 0 } else { n }).min(out.ids[3]);
            out.births[e] = total;
        }
        out
    }
}
