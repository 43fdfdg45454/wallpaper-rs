use super::budget::{CPU_BOIDS, MAX_ROPE, cells};
use super::kept::{Kept, asked, flocks as boids, roped};
use super::pack::{EVENT, PARTICLE_EVENTS};
use super::{Flock, MAX_EMITTERS2, Moment, Rope, System, Wall, flocks};
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
    pub strides: Vec<u32>,
    pub longs: Vec<bool>,
    pub marks: Vec<u32>,
    pub layers: Vec<[[u32; 2]; MAX_EMITTERS2]>,
    pub walls: Vec<[Wall; super::MAX_COLLIDERS2]>,
    pub ropes: Vec<Option<Rope>>,
    pub chains: Vec<u32>,
    pub grids: Vec<[u32; 3]>,
    pub tier: Option<Tier>,
    pub(super) fields: Vec<[[f32; 4]; super::MAX_COLLIDERS2]>,
    asked: Vec<Kept>,
    makes: Vec<u32>,
    ons: Vec<Option<Moment>>,
    pub(super) ranges: Vec<[[f32; 4]; MAX_EMITTERS2]>,
    pub(super) links: Vec<[u32; 4]>,
}

struct Room {
    alloc: u32,
    makes: u32,
    trail: u32,
    rope: u32,
    cells: u32,
}

fn layout(r: &Room) -> ([u32; 4], [u32; 4], u32) {
    let events = if r.makes & 3 != 0 { 2 * PARTICLE_EVENTS * EVENT as u32 } else { 0 };
    let mirror = if r.makes & 4 != 0 { r.alloc * EVENT as u32 } else { 0 };
    let marks = r.alloc + events + mirror;
    let pad = if r.trail > 0 { 8 } else { 0 };
    let chain = marks + r.alloc * r.trail * 4 + pad;
    let grid = chain + r.rope;
    let more = if r.cells > 0 { 3 * r.cells + 3 * r.alloc } else { 0 };
    let links = [r.alloc, r.alloc + PARTICLE_EVENTS * EVENT as u32, r.alloc + events, marks];
    (links, [chain, grid, grid + 3 * r.cells + r.alloc, 0], 4 + grid + more)
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
        let mut bake = Bake { tier: Some(tier), ..Bake::default() };
        let [w, h] = scene.canvas.size.map(|v| v as f32);
        let on = |s: &System, m| s.children.iter().any(|c| c.on == m);
        for part in &f.parts {
            let s = systems[part.system];
            let alloc =
                (u64::from(s.capacity) * cap / total.max(cap)).clamp(1, u64::from(s.capacity.max(1))) as u32;
            let alloc = if cpu && boids(s) { alloc.min(CPU_BOIDS) } else { alloc };
            let spread =
                s.overdraw(scene.canvas.size) * alloc as f32 / s.capacity.max(1) as f32 * draws(f) as f32;
            bake.allocs.push(alloc);
            bake.sizes.push((tier.overdraw() / spread.max(1e-9)).sqrt().min(1.0));
            bake.hosts.push(part.host);
            bake.ons.push(part.on);
            bake.systems.push(part.system);
            bake.asked.push(asked(scene, ids[part.system], s));
            bake.ropes.push(roped(scene, ids[part.system], s).filter(|_| part.host.is_none()));
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
            bake.walls.push(super::wall::walls(s, &mut bake.points));
            bake.grids.push([0, if boids(s) { cells(alloc) } else { 0 }, 0]);
        }
        bake.retrail(tier);
        Some(bake)
    }

    pub fn retrail(&mut self, tier: Tier) {
        self.tier = Some(tier);
        let n = self.allocs.len();
        let cap = |k: usize| MAX_TRAIL / self.allocs[k].max(1);
        let kept: Vec<Kept> = (0..n).map(|k| self.asked[k].capped(tier, cap(k))).collect();
        self.trails = kept.iter().map(|k| k.kept).collect();
        self.strides = kept.iter().map(|k| k.stride).collect();
        self.longs = kept.iter().map(|k| k.long).collect();
        let rooms = (0..n).map(|k| Room {
            alloc: self.allocs[k],
            makes: self.makes[k],
            trail: self.trails[k],
            rope: if self.ropes[k].is_some() { self.allocs[k].min(MAX_ROPE) } else { 0 },
            cells: self.grids[k][1],
        });
        let shapes: Vec<_> = rooms.map(|r| layout(&r)).collect();
        (self.links, self.states, self.marks, self.chains) = Default::default();
        for (k, ([spawn, death, mirror, marks], [chain, grid, steer, _], size)) in shapes.iter().enumerate() {
            let host = self.hosts[k].map_or(0, |h| match self.ons[k] {
                Some(Moment::Spawn) => shapes[h].0[0],
                Some(Moment::Death) => shapes[h].0[1],
                _ => shapes[h].0[2],
            });
            self.links.push([*spawn, *death, *mirror, host]);
            self.states.push(*size);
            self.marks.push(*marks);
            self.chains.push(*chain);
            (self.grids[k][0], self.grids[k][2]) = (*grid, *steer);
        }
    }

    pub fn alloc(&self) -> u32 {
        self.allocs.iter().sum()
    }
}
