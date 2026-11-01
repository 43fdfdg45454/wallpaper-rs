use super::cloud::Particle;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Census {
    pub alive: u32,
    pub min: [f32; 2],
    pub max: [f32; 2],
    pub mean: [f32; 2],
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

pub fn seen(p: &Particle, deep: [f32; 4]) -> ([f32; 2], f32) {
    if deep[3] < 0.5 {
        return (p.pos, 1.0);
    }
    let f = deep[0] + p.depth[0];
    let k = if f > 1.0 { deep[0] / f } else { 0.0 };
    ([deep[1] + (p.pos[0] - deep[1]) * k, deep[2] + (p.pos[1] - deep[2]) * k], k)
}

pub fn mark(p: &Particle, deep: [f32; 4]) -> [u32; 4] {
    let half = |v: f32| u32::from(half::f16::from_f32(v).to_bits());
    let (pos, k) = seen(p, deep);
    let byte = |c: f32| (c.clamp(0.0, 1.0) * 255.0).round_ties_even() as u32;
    let c = [p.color[0], p.color[1], p.color[2], p.color[3] * p.look[0]];
    let packed = c.iter().enumerate().map(|(i, c)| byte(*c) << (8 * i)).sum();
    let t = (p.age / p.life).min(1.0);
    [pos[0].to_bits(), pos[1].to_bits(), half(p.size * p.look[1] * k) | half(t) << 16, packed]
}
