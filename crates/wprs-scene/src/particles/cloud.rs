use super::Sim;
use super::motion::simulate;
use super::pack::PARTICLE_EVENTS;
use super::spawn::{hatch, spawn};
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
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Pod, Zeroable)]
pub struct Event {
    pub pos: [f32; 2],
    pub vel: [f32; 2],
    pub color: [f32; 4],
    pub size: f32,
    pub rot: f32,
    pub id: u32,
    pub slot: u32,
    pub prev: [f32; 2],
    pub alive: u32,
    pub pad: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Census {
    pub alive: u32,
    pub min: [f32; 2],
    pub max: [f32; 2],
    pub mean: [f32; 2],
}

#[derive(Debug, Clone, Default)]
pub struct Cloud {
    pub particles: Vec<Particle>,
    free: Vec<u32>,
    pub(super) spawned: Vec<Event>,
    pub(super) died: Vec<Event>,
    pub(super) mirror: Vec<Event>,
    pub(super) hatched: u32,
}

impl Event {
    pub(super) fn of(p: &Particle, slot: usize, prev: [f32; 2]) -> Event {
        let (pos, vel, color, size, rot, id) = (p.pos, p.vel, p.color, p.size, p.rot, p.id);
        let (slot, alive) = (slot as u32, u32::from(p.age < p.life));
        Event { pos, vel, color, size, rot, id, slot, prev, alive, pad: 0 }
    }
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
        for (i, p) in self.particles.iter_mut().enumerate().filter(|(_, p)| p.age < p.life) {
            let prev = p.pos;
            simulate(p, s, mirror);
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
        match (s.mode[0], host) {
            (0, _) => (0..s.total()).for_each(|k| {
                let e = (0..7).find(|e| k < s.births[*e]).unwrap_or(7);
                let p = spawn(s, e, s.ids[1].wrapping_add(k), points, None);
                self.place(s, p);
            }),
            (1, Some(h)) => h.spawned.iter().for_each(|ev| hatch(self, s, ev, points)),
            (2, Some(h)) => h.died.iter().for_each(|ev| hatch(self, s, ev, points)),
            (3, Some(h)) => {
                h.mirror.iter().filter(|ev| ev.alive != 0).for_each(|ev| hatch(self, s, ev, points))
            }
            _ => {}
        }
    }

    pub(super) fn place(&mut self, s: &Sim, p: Particle) -> bool {
        let floor = s.counts[0].saturating_sub(s.ids[3]) as usize;
        if self.free.len() <= floor {
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

impl Census {
    pub fn of(particles: &[Particle]) -> Census {
        let mut c = Census { min: [f32::MAX; 2], max: [f32::MIN; 2], ..Census::default() };
        let mut sum = [0.0f64; 2];
        for p in particles.iter().filter(|p| p.age < p.life) {
            c.alive += 1;
            for (i, (sum, at)) in sum.iter_mut().zip(p.pos).enumerate() {
                (c.min[i], c.max[i], *sum) = (c.min[i].min(at), c.max[i].max(at), *sum + f64::from(at));
            }
        }
        c.mean = sum.map(|v| (v / f64::from(c.alive.max(1))) as f32);
        c
    }

    pub fn join(parts: &[Census]) -> Census {
        let mut c = Census { min: [f32::MAX; 2], max: [f32::MIN; 2], ..Census::default() };
        let mut sum = [0.0f64; 2];
        for p in parts.iter().filter(|p| p.alive > 0) {
            c.alive += p.alive;
            for (i, sum) in sum.iter_mut().enumerate() {
                (c.min[i], c.max[i]) = (c.min[i].min(p.min[i]), c.max[i].max(p.max[i]));
                *sum += f64::from(p.mean[i]) * f64::from(p.alive);
            }
        }
        c.mean = sum.map(|v| (v / f64::from(c.alive.max(1))) as f32);
        c
    }
}
