use super::Sim;
use super::census::mark;
use super::hatch::Event;
use super::motion::simulate;
use super::pack::PARTICLE_EVENTS;
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Pod, Zeroable)]
pub struct Particle {
    pub pos: [f32; 2],
    pub vel: [f32; 2],
    pub age: f32,
    pub life: f32,
    pub size: f32,
    pub rot: f32,
    pub spin: f32,
    pub frame: f32,
    pub rand: f32,
    pub id: u32,
    pub color: [f32; 4],
    pub host: [u32; 2],
    pub look: [f32; 2],
    pub depth: [f32; 2],
    pub spare: [f32; 2],
}

#[derive(Debug, Clone, Default)]
pub struct Cloud {
    pub particles: Vec<Particle>,
    free: Vec<u32>,
    pub(super) spawned: Vec<Event>,
    pub(super) died: Vec<Event>,
    pub(super) mirror: Vec<Event>,
    pub(super) hatched: u32,
    pub trail: Vec<[u32; 4]>,
    pub rope: Vec<super::Strand>,
    school: super::school::School,
    order: Vec<(u32, u32)>,
}

impl Cloud {
    pub fn new(capacity: u32) -> Cloud {
        Cloud {
            particles: vec![Particle::default(); capacity as usize],
            free: (0..capacity).rev().collect(),
            ..Cloud::default()
        }
    }

    pub fn clear(&mut self) {
        let n = self.particles.len() as u32;
        *self = Cloud::new(n);
    }

    pub fn alive(&self) -> u32 {
        (self.particles.len() - self.free.len()) as u32
    }

    pub fn step(&mut self, s: &Sim, points: &[[f32; 2]]) {
        Cloud::brood(std::slice::from_mut(self), std::slice::from_ref(s), &[None], points);
    }

    pub fn brood(clouds: &mut [Cloud], sims: &[Sim], hosts: &[Option<usize>], points: &[[f32; 2]]) {
        for (p, sim) in sims.iter().enumerate().take(clouds.len()) {
            let (done, rest) = clouds.split_at_mut(p);
            let host = hosts.get(p).copied().flatten().and_then(|h| done.get(h));
            rest[0].advance(sim, host, points);
        }
        clouds.iter_mut().for_each(|c| c.died.clear());
    }

    fn advance(&mut self, s: &Sim, host: Option<&Cloud>, points: &[[f32; 2]]) {
        let (makes, n) = (s.mode[1], self.particles.len());
        self.hatched = 0;
        self.spawned.clear();
        if makes & 4 != 0 && self.mirror.len() != n {
            self.mirror = vec![Event::default(); n];
        }
        let mirror = host.map_or(&[][..], |h| h.mirror.as_slice());
        let (k, stride) = (s.trail[1] as usize, s.trail[2].max(1));
        if self.trail.len() != n * k {
            self.trail = vec![[0; 4]; n * k];
        }
        self.school.steer(&self.particles, s);
        let span = stride as f32 / 60.0;
        for (i, p) in self.particles.iter_mut().enumerate().filter(|(_, p)| p.age < p.life) {
            let (prev, before) = (p.pos, p.age);
            let nudge = self.school.nudges.get(i).copied().filter(|_| s.flock[1] > 0).unwrap_or_default();
            simulate(p, s, mirror, points, nudge);
            if let Some(h) = self.trail.get_mut(i * k..(i + 1) * k).filter(|_| k > 0) {
                if stride == 1 || (p.age / span) as u32 != (before / span) as u32 {
                    h.copy_within(..k - 1, 1);
                }
                h[0] = mark(p, s.depth[2]);
                if p.age >= p.life {
                    h.iter_mut().for_each(|e| e[2] = 0);
                }
            }
            if makes & 4 != 0 {
                self.mirror[i] = Event::of(p, i, prev);
            }
            if p.age >= p.life {
                self.free.push(i as u32);
                if makes & 2 != 0 && self.died.len() < PARTICLE_EVENTS as usize {
                    self.died.push(Event::of(p, i, prev));
                }
            }
        }
        self.breed(s, host, points);
    }

    pub fn strand(&mut self, s: &Sim) {
        super::strand::strung(&self.particles, s, &mut self.order, &mut self.rope);
    }

    pub(super) fn full(&self, s: &Sim) -> bool {
        self.free.len() <= s.counts[0].saturating_sub(s.ids[3]) as usize
    }

    pub(super) fn place(&mut self, s: &Sim, p: Particle) -> bool {
        if self.full(s) {
            return false;
        }
        let slot = self.free.pop().unwrap_or_default() as usize;
        self.particles[slot] = p;
        if s.mode[1] & 1 != 0 && self.spawned.len() < PARTICLE_EVENTS as usize {
            self.spawned.push(Event::of(&p, slot, p.pos));
        }
        true
    }
}
