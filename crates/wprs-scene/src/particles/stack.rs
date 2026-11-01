use super::{Particle, Render, System};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Stack {
    Newest,
    Oldest,
    Largest,
    Smallest,
    Lowest,
    #[serde(skip)]
    Farthest,
}

pub fn bin(p: &Particle, kind: Stack, size: f32, height: f32) -> u32 {
    let t = (p.age / p.life).clamp(0.0, 1.0);
    let s = (p.size * p.look[1] / size.max(1e-6)).clamp(0.0, 1.0);
    let k = match kind {
        Stack::Newest => 1.0 - t,
        Stack::Oldest => t,
        Stack::Largest => s,
        Stack::Smallest => 1.0 - s,
        Stack::Lowest => (p.pos[1] / height.max(1.0)).clamp(0.0, 1.0),
        Stack::Farthest => (0.5 - p.depth[0] / (2.0 * size.max(1e-6))).clamp(0.0, 1.0),
    };
    ((k * 256.0) as u32).min(255)
}

pub fn stacked(particles: &[Particle], (kind, size, height): (Stack, f32, f32), out: &mut Vec<Particle>) {
    let live =
        || particles.iter().filter(|p| p.age < p.life).map(|p| (bin(p, kind, size, height) as usize, p));
    let mut at = [0usize; 257];
    live().for_each(|(b, _)| at[b + 1] += 1);
    (1..at.len()).for_each(|i| at[i] += at[i - 1]);
    out.resize(particles.len(), Particle::default());
    out[at[256]..].fill(Particle::default());
    for (b, p) in live() {
        out[at[b]] = *p;
        at[b] += 1;
    }
}

pub fn largest(s: &System, r: &Render) -> f32 {
    let peak = r.size_over_life.iter().map(|k| k[1]).fold(0.0, f32::max);
    s.init.size.ends[1].max(s.init.size.ends[0]) * if r.size_over_life.is_empty() { 1.0 } else { peak }
}
