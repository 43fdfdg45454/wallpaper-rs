use super::{Force, Particle, Sim, System};

impl Sim {
    pub(super) fn school(&mut self, s: &System) {
        let first = s.forces.iter().find_map(|f| match &f.force {
            Force::Boids(b) => Some(*b),
            _ => None,
        });
        if let Some(b) = first {
            self.boids =
                [[b.radius.max(1.0), b.separation, b.alignment, b.cohesion], [b.max_speed, 1.0, 0.0, 0.0]];
        }
    }
}

pub fn hash(c: [i32; 2], cells: u32) -> u32 {
    ((c[0] as u32).wrapping_mul(73_856_093) ^ (c[1] as u32).wrapping_mul(19_349_663)) & (cells - 1)
}

fn cell(p: [f32; 2], r: f32) -> [i32; 2] {
    [(p[0] / r).floor() as i32, (p[1] / r).floor() as i32]
}

#[derive(Debug, Clone, Default)]
pub(super) struct School {
    counts: Vec<u32>,
    starts: Vec<u32>,
    fill: Vec<u32>,
    sorted: Vec<u32>,
    pub(super) nudges: Vec<[f32; 2]>,
}

impl School {
    pub(super) fn steer(&mut self, particles: &[Particle], s: &Sim) {
        let (cells, n, r) = (s.flock[1] as usize, particles.len(), s.boids[0][0]);
        if cells == 0 {
            return;
        }
        self.counts.resize(cells, 0);
        self.counts.fill(0);
        self.starts.resize(cells, 0);
        self.sorted.resize(n, 0);
        self.nudges.resize(n, [0.0; 2]);
        let alive = |p: &&Particle| p.age < p.life;
        for p in particles.iter().filter(alive) {
            self.counts[hash(cell(p.pos, r), cells as u32) as usize] += 1;
        }
        let mut at = 0;
        for (start, count) in self.starts.iter_mut().zip(&self.counts) {
            (*start, at) = (at, at + count);
        }
        self.fill.clone_from(&self.starts);
        for (i, p) in particles.iter().enumerate().filter(|(_, p)| p.age < p.life) {
            let h = hash(cell(p.pos, r), cells as u32) as usize;
            self.sorted[self.fill[h] as usize] = i as u32;
            self.fill[h] += 1;
        }
        for (i, p) in particles.iter().enumerate() {
            self.nudges[i] = if p.age < p.life { self.near(particles, s, i) } else { [0.0; 2] };
        }
    }

    fn near(&self, all: &[Particle], s: &Sim, i: usize) -> [f32; 2] {
        let ([r, sep, align, coh], cap, cells) = (s.boids[0], s.flock[2], s.flock[1]);
        let p = &all[i];
        let c = cell(p.pos, r);
        let (mut n, mut at, mut vel, mut away) = (0u32, [0.0f32; 2], [0.0f32; 2], [0.0f32; 2]);
        let mut seen = [u32::MAX; 9];
        'cells: for m in 0..9 {
            let h = hash([c[0] + m as i32 % 3 - 1, c[1] + m as i32 / 3 - 1], cells);
            if seen.contains(&h) {
                continue;
            }
            seen[m] = h;
            let start = self.starts[h as usize] as usize;
            for &j in &self.sorted[start..start + self.counts[h as usize] as usize] {
                let q = &all[j as usize];
                let d = [p.pos[0] - q.pos[0], p.pos[1] - q.pos[1]];
                let l = (d[0] * d[0] + d[1] * d[1]).sqrt();
                if j as usize == i || l >= r || l < 1e-6 {
                    continue;
                }
                n += 1;
                let w = (1.0 - l / r) / l;
                at = [at[0] + q.pos[0], at[1] + q.pos[1]];
                vel = [vel[0] + q.vel[0], vel[1] + q.vel[1]];
                away = [away[0] + d[0] * w, away[1] + d[1] * w];
                if n >= cap {
                    break 'cells;
                }
            }
        }
        if n == 0 {
            return [0.0; 2];
        }
        let n = n as f32;
        [0, 1].map(|a| away[a] / n * sep * r + (vel[a] / n - p.vel[a]) * align + (at[a] / n - p.pos[a]) * coh)
    }
}
