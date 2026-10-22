use super::Sim;
use super::motion::simulate;
use super::spawn::spawn;
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
}

impl Cloud {
    pub fn new(capacity: u32) -> Cloud {
        Cloud { particles: vec![Particle::default(); capacity as usize], free: (0..capacity).rev().collect() }
    }

    pub fn clear(&mut self) {
        self.particles.fill(Particle::default());
        self.free.clear();
        self.free.extend((0..self.particles.len() as u32).rev());
    }

    pub fn alive(&self) -> u32 {
        (self.particles.len() - self.free.len()) as u32
    }

    pub fn step(&mut self, s: &Sim, points: &[[f32; 2]]) {
        for (i, p) in self.particles.iter_mut().enumerate().filter(|(_, p)| p.age < p.life) {
            simulate(p, s);
            if p.age >= p.life {
                self.free.push(i as u32);
            }
        }
        let floor = s.counts[0].saturating_sub(s.ids[3]) as usize;
        for k in 0..s.births[3] {
            if self.free.len() <= floor {
                break;
            }
            let slot = self.free.pop().unwrap_or_default() as usize;
            self.particles[slot] = spawn(s, k, points);
        }
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
}
