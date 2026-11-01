use super::field::MAX_FIELD;
use super::{MAX_COLLIDERS2, Surface, System};

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Wall {
    pub start: u32,
    pub cells: [u32; 2],
    pub map: [f32; 2],
    pub place: [f32; 6],
    pub ready: bool,
}

pub(super) fn walls(s: &System, points: &mut Vec<[f32; 2]>) -> [Wall; MAX_COLLIDERS2] {
    let mut out = [Wall::default(); MAX_COLLIDERS2];
    for (k, c) in s.collide.iter().take(MAX_COLLIDERS2).enumerate() {
        if matches!(c.bounds, Surface::Layer { .. }) {
            out[k].start = points.len() as u32;
            points.resize(points.len() + MAX_FIELD, [0.0; 2]);
        }
    }
    out
}

impl Wall {
    pub(super) fn fit(&self, slots: &mut [[f32; 4]]) {
        if !self.ready {
            slots[0][0] = 0.0;
            return;
        }
        let [a, b] = [slots[1], slots[2]];
        let m = crate::Affine([a[0], a[1], a[2], a[3], b[0], b[1]]);
        let into = crate::Affine(self.place).then(m.inverse()).0;
        slots[0] = [7.0, self.start as f32, self.cells[0] as f32, self.cells[1] as f32];
        slots[1] = [into[0], into[1], into[2], into[3]];
        slots[2] = [into[4], into[5], self.map[0], self.map[1]];
    }
}
