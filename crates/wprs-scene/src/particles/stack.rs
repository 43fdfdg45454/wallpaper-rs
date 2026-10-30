use super::{Particle, Render, Stack, System};

pub fn bin(p: &Particle, kind: Stack, size: f32, height: f32) -> u32 {
    let t = (p.age / p.life).clamp(0.0, 1.0);
    let s = (p.size * p.look[1] / size.max(1e-6)).clamp(0.0, 1.0);
    let k = match kind {
        Stack::Newest => 1.0 - t,
        Stack::Oldest => t,
        Stack::Largest => s,
        Stack::Smallest => 1.0 - s,
        Stack::Lowest => (p.pos[1] / height.max(1.0)).clamp(0.0, 1.0),
    };
    ((k * 256.0) as u32).min(255)
}

pub fn stacked(particles: &[Particle], kind: Stack, size: f32, height: f32) -> Vec<Particle> {
    let mut out: Vec<Particle> = particles.iter().filter(|p| p.age < p.life).copied().collect();
    out.sort_by_key(|p| bin(p, kind, size, height));
    out.resize(particles.len(), Particle::default());
    out
}

pub fn largest(s: &System, r: &Render) -> f32 {
    let peak = r.size_over_life.iter().map(|k| k[1]).fold(0.0, f32::max);
    s.init.size.ends[1].max(s.init.size.ends[0]) * if r.size_over_life.is_empty() { 1.0 } else { peak }
}
