use super::pack::{EVENT, PARTICLE_EVENTS};
use super::{EmitShape, Flock, MAX_EMITTERS2, Sim, System, flocks};
use crate::{Kind, Scene, Tier};

pub const CPU_PARTICLES: u32 = 5_000;
pub const MAX_POINTS: usize = 4096;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Bake {
    pub allocs: Vec<u32>,
    pub sizes: Vec<f32>,
    pub states: Vec<u32>,
    pub hosts: Vec<Option<usize>>,
    pub systems: Vec<usize>,
    pub trail: u32,
    pub points: Vec<[f32; 2]>,
    ranges: Vec<[[f32; 4]; MAX_EMITTERS2]>,
    links: Vec<[u32; 4]>,
}

fn layout(alloc: u32, makes: u32) -> ([u32; 3], u32) {
    let events = if makes & 3 != 0 { 2 * PARTICLE_EVENTS * EVENT as u32 } else { 0 };
    let mirror = if makes & 4 != 0 { alloc * EVENT as u32 } else { 0 };
    ([alloc, alloc + PARTICLE_EVENTS * EVENT as u32, alloc + events], 4 + alloc + events + mirror)
}

impl Bake {
    pub fn new<'a>(
        scene: &Scene,
        flock: usize,
        tier: Tier,
        cpu: bool,
        image: impl Fn(&str) -> Option<(u32, u32, &'a [u8])>,
    ) -> Option<Bake> {
        let all = flocks(scene);
        let f: &Flock = all.get(flock)?;
        let systems: Vec<&System> = scene.particles.values().collect();
        let ids: Vec<&String> = scene.particles.keys().collect();
        let draws = |f: &Flock| {
            let id = ids[f.system];
            let shared =
                scene.layers().filter(|l| matches!(&l.kind, Kind::Particles { system, .. } if system == id));
            if f.layer.is_some() { 1 } else { shared.count().max(1) as u64 }
        };
        let weight =
            |f: &Flock| f.parts.iter().map(|p| u64::from(systems[p.system].capacity)).sum::<u64>() * draws(f);
        let total: u64 = all.iter().map(weight).sum();
        let cap = u64::from(if cpu { tier.particles().min(CPU_PARTICLES) } else { tier.particles() });
        let mut bake = Bake { trail: tier.trail(), ..Bake::default() };
        let [w, h] = scene.canvas.size.map(|v| v as f32);
        for part in &f.parts {
            let s = systems[part.system];
            let alloc =
                (u64::from(s.capacity) * cap / total.max(cap)).clamp(1, u64::from(s.capacity.max(1))) as u32;
            let spread =
                s.overdraw(scene.canvas.size) * alloc as f32 / s.capacity.max(1) as f32 * draws(f) as f32;
            bake.allocs.push(alloc);
            bake.sizes.push((tier.overdraw() / spread.max(1e-9)).sqrt().min(1.0));
            bake.hosts.push(part.host);
            bake.systems.push(part.system);
            let mut ranges = [[0.0; 4]; MAX_EMITTERS2];
            for (e, em) in s.emitters.iter().take(MAX_EMITTERS2).enumerate() {
                let EmitShape::Mask { texture, threshold } = &em.shape else { continue };
                let Some((iw, ih, rgba)) = image(texture) else { continue };
                let stride = ((iw as f32 * ih as f32 / MAX_POINTS as f32).sqrt().ceil() as usize).max(1);
                let start = bake.points.len();
                let at = |v: usize, n: u32, side: f32| (v as f32 + stride as f32 / 2.0) / n as f32 * side;
                for y in (0..ih as usize).step_by(stride) {
                    for x in (0..iw as usize).step_by(stride) {
                        let alpha = rgba.get((y * iw as usize + x) * 4 + 3).copied().unwrap_or(0);
                        if f32::from(alpha) / 255.0 > threshold.0 {
                            bake.points.push([at(x, iw, w), at(y, ih, h)]);
                        }
                    }
                }
                let cell = [stride as f32 / iw as f32 * w, stride as f32 / ih as f32 * h];
                ranges[e] = [start as f32, (bake.points.len() - start) as f32, cell[0], cell[1]];
            }
            bake.ranges.push(ranges);
        }
        let makes = |s: &System| {
            let on = |m| s.children.iter().any(|c| c.on == m);
            u32::from(on(super::Moment::Spawn))
                + 2 * u32::from(on(super::Moment::Death))
                + 4 * u32::from(on(super::Moment::Follow))
        };
        let shapes: Vec<_> =
            f.parts.iter().zip(&bake.allocs).map(|(p, a)| layout(*a, makes(systems[p.system]))).collect();
        for (k, part) in f.parts.iter().enumerate() {
            let ([spawn, death, mirror], size) = shapes[k];
            let host = part.host.map_or(0, |h| match part.on {
                Some(super::Moment::Spawn) => shapes[h].0[0],
                Some(super::Moment::Death) => shapes[h].0[1],
                _ => shapes[h].0[2],
            });
            bake.links.push([spawn, death, mirror, host]);
            bake.states.push(size);
        }
        Some(bake)
    }

    pub fn alloc(&self) -> u32 {
        self.allocs.iter().sum()
    }

    pub fn apply(&self, part: usize, sim: &Sim) -> Sim {
        let mut out = *sim;
        let alloc = self.allocs[part];
        out.counts[0] = alloc;
        out.ids[3] = sim.ids[3].min(alloc);
        out.links = self.links[part];
        out.mode[3] = self.hosts[part].map_or(0, |h| self.allocs[h]);
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
            total = total
                .saturating_add(if mask && self.ranges[part][e][1] < 1.0 { 0 } else { n })
                .min(out.ids[3]);
            out.births[e] = total;
        }
        out
    }
}
