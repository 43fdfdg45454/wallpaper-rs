use crate::{Affine, Lean};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub struct AlphaMap {
    pub size: [u32; 2],
    pub alpha: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hitbox {
    pub layer: String,
    pub to_box: Affine,
    pub to_local: Affine,
    pub cell: [f32; 4],
    pub alpha: f32,
    pub threshold: Option<f32>,
    pub coverage: Option<Arc<AlphaMap>>,
    pub unlean: Option<Lean>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Hitboxes(pub Vec<Hitbox>);

impl AlphaMap {
    pub fn at(&self, [u, v]: [f32; 2]) -> f32 {
        let [w, h] = self.size;
        let x = ((u * w as f32) as u32).min(w.saturating_sub(1));
        let y = ((v * h as f32) as u32).min(h.saturating_sub(1));
        self.alpha.get((y * w + x) as usize).map_or(0.0, |a| f32::from(*a) / 255.0)
    }
}

impl Hitbox {
    pub fn flat(&self, p: [f32; 2]) -> [f32; 2] {
        self.unlean.map_or(p, |l| l.apply(p))
    }

    pub fn holds(&self, p: [f32; 2]) -> bool {
        let u = self.to_box.apply(self.flat(p));
        let seen = || {
            let [x, y, w, h] = self.cell;
            self.coverage.as_ref().map_or(1.0, |c| c.at([x + u[0] * w, y + u[1] * h]))
        };
        u.iter().all(|c| (0.0..=1.0).contains(c)) && self.threshold.is_none_or(|t| self.alpha * seen() >= t)
    }
}

impl Hitboxes {
    pub fn hit(&self, p: [f32; 2]) -> Option<(&str, [f32; 2])> {
        self.0.iter().rev().find(|b| b.holds(p)).map(|b| (b.layer.as_str(), b.to_local.apply(b.flat(p))))
    }

    pub fn local(&self, layer: &str, p: [f32; 2]) -> [f32; 2] {
        self.0.iter().rfind(|b| b.layer == layer).map_or([0.0; 2], |b| b.to_local.apply(b.flat(p)))
    }
}
