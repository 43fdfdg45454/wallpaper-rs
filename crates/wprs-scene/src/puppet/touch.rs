use super::Puppet;
use super::limb::{end, frame};
use crate::MeshLayer;
use crate::rigged::VERTEX_FLOATS;

type Skinned = ([f32; 2], [(usize, f32); 4]);

impl Puppet {
    fn skinned(&self, i: u32) -> Skinned {
        let v = &self.rig.vertices[i as usize * VERTEX_FLOATS..][..VERTEX_FLOATS];
        let (mut at, mut joints) = ([0.0f32; 2], [(0usize, 0.0f32); 4]);
        for (k, slot) in joints.iter_mut().enumerate() {
            *slot = (v[5 + k] as usize, v[9 + k]);
            let Some(m) = self.posed.palette.get(slot.0).filter(|_| slot.1 != 0.0) else { continue };
            let q = end(m, [v[0], v[1], v[2]]);
            (at[0], at[1]) = (at[0] + q[0] * slot.1, at[1] + q[1] * slot.1);
        }
        (at, joints)
    }

    pub fn bone_at(&self, m: &MeshLayer, [x, y]: [f32; 2]) -> Option<&str> {
        if m.rig.hit.is_empty() {
            return None;
        }
        let rig = &*self.rig;
        for d in self.posed.draws.iter().rev() {
            let piece = rig.pieces.get(d.piece as usize)?;
            let list = rig.indices.get(piece.first as usize..(piece.first + piece.count) as usize)?;
            for tri in list.as_chunks::<3>().0 {
                let [a, b, c] = [tri[0], tri[1], tri[2]].map(|i| self.skinned(i));
                let ([x0, y0], [x1, y1], [x2, y2]) = (a.0, b.0, c.0);
                let det = (y1 - y2) * (x0 - x2) + (x2 - x1) * (y0 - y2);
                if det.abs() < 1e-9 {
                    continue;
                }
                let u = ((y1 - y2) * (x - x2) + (x2 - x1) * (y - y2)) / det;
                let v = ((y2 - y0) * (x - x2) + (x0 - x2) * (y - y2)) / det;
                if u < 0.0 || v < 0.0 || u + v > 1.0 {
                    continue;
                }
                let spread = [(u, a.1), (v, b.1), (1.0 - u - v, c.1)];
                let weigh = |j: usize| -> f32 {
                    spread
                        .iter()
                        .flat_map(|(f, js)| js.iter().filter(|s| s.0 == j).map(move |s| f * s.1))
                        .sum()
                };
                let all = spread.iter().flat_map(|(_, js)| js.iter().map(|s| s.0));
                let best = all.max_by(|p, q| weigh(*p).total_cmp(&weigh(*q)))?;
                let node = rig.palette.get(best)?.0 as usize;
                let named = |n: &usize| m.rig.hit.contains(&rig.joints[*n].name);
                let up = std::iter::successors(Some(node), |n| rig.joints[*n].parent.map(usize::from));
                return up.take(rig.joints.len()).find(named).map(|n| rig.joints[n].name.as_str());
            }
        }
        None
    }

    pub fn bone_frames(&self, size: [f32; 2]) -> Vec<[f32; 6]> {
        self.worlds.iter().map(|w| frame(w, size).0).collect()
    }
}
