use super::pack::{EVENT, PARTICLE_EVENTS};
use super::{Flock, MAX_EMITTERS2, Moment, Sim, System, flocks};
use crate::{Kind, Scene, Tier};

pub const CPU_PARTICLES: u32 = 5_000;
pub const MAX_POINTS: usize = 4096;
pub const MAX_TRAIL: u32 = 1 << 20;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Bake {
    pub allocs: Vec<u32>,
    pub sizes: Vec<f32>,
    pub states: Vec<u32>,
    pub hosts: Vec<Option<usize>>,
    pub systems: Vec<usize>,
    pub points: Vec<[f32; 2]>,
    pub trails: Vec<u32>,
    pub marks: Vec<u32>,
    pub layers: Vec<[[u32; 2]; MAX_EMITTERS2]>,
    fields: Vec<[[f32; 4]; super::MAX_COLLIDERS2]>,
    asked: Vec<u32>,
    makes: Vec<u32>,
    ons: Vec<Option<Moment>>,
    ranges: Vec<[[f32; 4]; MAX_EMITTERS2]>,
    links: Vec<[u32; 4]>,
}

fn layout(alloc: u32, makes: u32, trail: u32) -> ([u32; 4], u32) {
    let events = if makes & 3 != 0 { 2 * PARTICLE_EVENTS * EVENT as u32 } else { 0 };
    let mirror = if makes & 4 != 0 { alloc * EVENT as u32 } else { 0 };
    let marks = alloc + events + mirror;
    ([alloc, alloc + PARTICLE_EVENTS * EVENT as u32, alloc + events, marks], 4 + marks + alloc * trail * 4)
}

fn asked(scene: &Scene, id: &str, s: &System) -> u32 {
    let own = scene.layers().filter_map(|l| match &l.kind {
        Kind::Particles { system, render: Some(r), .. } if system == id => Some(&**r),
        _ => None,
    });
    let longest =
        std::iter::once(&s.render).chain(own).filter_map(|r| r.trail.as_ref()).map(|t| t.length).max();
    longest.unwrap_or(0).min(16)
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
        let mut bake = Bake::default();
        let [w, h] = scene.canvas.size.map(|v| v as f32);
        let on = |s: &System, m| s.children.iter().any(|c| c.on == m);
        for part in &f.parts {
            let s = systems[part.system];
            let alloc =
                (u64::from(s.capacity) * cap / total.max(cap)).clamp(1, u64::from(s.capacity.max(1))) as u32;
            let spread =
                s.overdraw(scene.canvas.size) * alloc as f32 / s.capacity.max(1) as f32 * draws(f) as f32;
            bake.allocs.push(alloc);
            bake.sizes.push((tier.overdraw() / spread.max(1e-9)).sqrt().min(1.0));
            bake.hosts.push(part.host);
            bake.ons.push(part.on);
            bake.systems.push(part.system);
            bake.asked.push(asked(scene, ids[part.system], s));
            bake.makes.push(
                [Moment::Spawn, Moment::Death, Moment::Follow]
                    .iter()
                    .zip([1, 2, 4])
                    .map(|(m, b)| if on(s, *m) { b } else { 0 })
                    .sum(),
            );
            bake.fields.push(super::field::fields(s, &image, [w, h], &mut bake.points));
            bake.ranges.push(super::field::shapes(s, &image, [w, h], &mut bake.points));
            bake.layers.push(super::field::reserve(s, &mut bake.points));
        }
        bake.retrail(tier);
        Some(bake)
    }

    pub fn retrail(&mut self, tier: Tier) {
        let n = self.allocs.len();
        let cap = |k: usize| MAX_TRAIL / self.allocs[k].max(1);
        self.trails = (0..n).map(|k| self.asked[k].min(tier.trail()).min(cap(k))).collect();
        let shapes: Vec<_> = (0..n).map(|k| layout(self.allocs[k], self.makes[k], self.trails[k])).collect();
        (self.links, self.states, self.marks) = Default::default();
        for (k, ([spawn, death, mirror, marks], size)) in shapes.iter().enumerate() {
            let host = self.hosts[k].map_or(0, |h| match self.ons[k] {
                Some(Moment::Spawn) => shapes[h].0[0],
                Some(Moment::Death) => shapes[h].0[1],
                _ => shapes[h].0[2],
            });
            self.links.push([*spawn, *death, *mirror, host]);
            self.states.push(*size);
            self.marks.push(*marks);
        }
    }

    pub fn alloc(&self) -> u32 {
        self.allocs.iter().sum()
    }

    pub fn apply(&self, part: usize, sim: &Sim) -> Sim {
        let mut out = *sim;
        let alloc = self.allocs[part];
        out.counts[0] = alloc;
        out.ids[3] = sim.ids[3].min(alloc);
        out.mode[2] = sim.mode[2].min(out.ids[3]);
        out.links = self.links[part];
        out.mode[3] = self.hosts[part].map_or(0, |h| self.allocs[h]);
        out.trail = [self.marks[part], self.trails[part], 0, 0];
        for (k, f) in self.fields[part].iter().enumerate() {
            if out.colliders[2 * k][0] == 4.0 {
                out.colliders[2 * k] = *f;
            }
        }
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
