use super::Skin;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use wprs_core::message::said;

pub const MAX_INFLUENCES: usize = 4;
const LEAST: f32 = 1e-4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Add,
    Subtract,
    Smooth,
    Normalise,
}

pub struct Brush {
    pub bone: usize,
    pub mode: Mode,
    pub by: f32,
    pub radius: f32,
    pub falloff: f32,
}

fn reach(d: f32, falloff: f32) -> f32 {
    let hard = 1.0 - falloff;
    match d {
        d if d >= 1.0 => 0.0,
        d if d <= hard => 1.0,
        d => {
            let s = (d - hard) / falloff.max(1e-6);
            1.0 - s * s * (3.0 - 2.0 * s)
        }
    }
}

pub fn fit(w: &mut Vec<(usize, f32)>, locked: &[usize]) {
    w.retain(|x| x.1 > LEAST);
    w.sort_by(|a, b| b.1.total_cmp(&a.1));
    while w.len() > MAX_INFLUENCES {
        let last = w.iter().rposition(|x| !locked.contains(&x.0)).unwrap_or(w.len() - 1);
        w.remove(last);
    }
    let held: f32 = w.iter().filter(|x| locked.contains(&x.0)).map(|x| x.1).sum();
    let free: f32 = w.iter().filter(|x| !locked.contains(&x.0)).map(|x| x.1).sum();
    let (share, from) = match free > LEAST {
        true => ((1.0 - held).max(0.0), free),
        false => (1.0, held + free),
    };
    for x in w.iter_mut().filter(|x| free <= LEAST || !locked.contains(&x.0)) {
        x.1 *= share / from.max(f32::MIN_POSITIVE);
    }
}

impl Skin {
    fn neighbours(&self) -> Vec<Vec<u32>> {
        let mut out = vec![Vec::new(); self.points.len()];
        for t in &self.triangles {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                out[a as usize].push(b);
                out[b as usize].push(a);
            }
        }
        out
    }

    fn shift(&self, w: &mut Vec<(usize, f32)>, b: &Brush, a: f32, locked: &[usize]) {
        let held: f32 = w.iter().filter(|x| locked.contains(&x.0)).map(|x| x.1).sum();
        let now = w.iter().find(|x| x.0 == b.bone).map_or(0.0, |x| x.1);
        let sign = if b.mode == Mode::Add { 1.0 } else { -1.0 };
        let new = (now + sign * a).clamp(0.0, (1.0 - held).max(0.0));
        let others: f32 = w.iter().filter(|x| x.0 != b.bone && !locked.contains(&x.0)).map(|x| x.1).sum();
        let left = (1.0 - held - new).max(0.0);
        let heir = self.up(b.bone).filter(|h| !locked.contains(h));
        let new = match (others > LEAST, heir) {
            (true, _) => {
                w.iter_mut()
                    .filter(|x| x.0 != b.bone && !locked.contains(&x.0))
                    .for_each(|x| x.1 *= left / others);
                new
            }
            (false, Some(h)) if left > LEAST => {
                w.push((h, left));
                new
            }
            _ => new + left,
        };
        w.retain(|x| x.0 != b.bone);
        w.push((b.bone, new));
    }

    pub fn paint(&mut self, b: &Brush, stroke: &[[f32; 2]], locked: &[usize]) -> Result<usize> {
        ensure!(!self.bones.is_empty(), said("editor.no_bones", "the mesh has no bones yet", &[]));
        self.settle();
        self.auto = false;
        let r = b.radius.max(1e-3);
        let near: Vec<f32> = self
            .points
            .iter()
            .map(|p| stroke.iter().map(|s| (s[0] - p[0]).hypot(s[1] - p[1]) / r).fold(1.0, f32::min))
            .collect();
        let around = if b.mode == Mode::Smooth { self.neighbours() } else { Vec::new() };
        let before = self.weights.clone();
        let mut touched = 0;
        for (v, d) in near.iter().enumerate() {
            let a = b.by * reach(*d, b.falloff);
            if a <= 0.0 {
                continue;
            }
            touched += 1;
            let mut w = before[v].clone();
            match b.mode {
                Mode::Add | Mode::Subtract => self.shift(&mut w, b, a, locked),
                Mode::Smooth if !around[v].is_empty() => {
                    let mut mean: BTreeMap<usize, f32> = BTreeMap::new();
                    for n in &around[v] {
                        for (k, x) in &before[*n as usize] {
                            *mean.entry(*k).or_default() += x / around[v].len() as f32;
                        }
                    }
                    for (k, _) in &w {
                        mean.entry(*k).or_insert(0.0);
                    }
                    let own = |k: usize| w.iter().find(|x| x.0 == k).map_or(0.0, |x| x.1);
                    let mixed = mean.iter().map(|(k, m)| match locked.contains(k) {
                        true => (*k, own(*k)),
                        false => (*k, own(*k) + (m - own(*k)) * a),
                    });
                    w = mixed.collect();
                }
                Mode::Smooth => {}
                Mode::Normalise => {
                    let best = w.iter().filter(|x| !locked.contains(&x.0)).max_by(|p, q| p.1.total_cmp(&q.1));
                    let best = best.map(|x| x.0);
                    w.retain(|x| locked.contains(&x.0) || x.1 >= a || Some(x.0) == best);
                }
            }
            fit(&mut w, locked);
            self.weights[v] = w;
        }
        Ok(touched)
    }
}
