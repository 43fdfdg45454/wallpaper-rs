use super::math::Mat;
use super::{Rigged, VERTEX_FLOATS};

pub(crate) fn stack(rig: &Rigged) -> Vec<Vec<(u16, [f32; 4])>> {
    let mut out: Vec<Vec<(u16, [f32; 4])>> = vec![Vec::new(); rig.parts.len()];
    let mut counts = vec![0.0f32; rig.parts.len()];
    for piece in &rig.pieces {
        let (part, list) = (piece.part as usize, &mut out[piece.part as usize]);
        for v in piece.base..piece.base + piece.vertices {
            let f = &rig.vertices[v as usize * VERTEX_FLOATS..][..VERTEX_FLOATS];
            counts[part] += 1.0;
            for k in (0..4).filter(|k| f[9 + k] > 0.0) {
                let (joint, w) = (f[5 + k] as u16, f[9 + k]);
                let at = match list.iter().position(|(j, _)| *j == joint) {
                    Some(i) => i,
                    None => {
                        list.push((joint, [0.0; 4]));
                        list.len() - 1
                    }
                };
                let sum = &mut list[at].1;
                [f[0], f[1], f[2], 1.0].iter().enumerate().for_each(|(c, x)| sum[c] += x * w);
            }
        }
    }
    for (list, n) in out.iter_mut().zip(counts) {
        list.iter_mut().for_each(|(_, s)| s.iter_mut().for_each(|x| *x /= n.max(1.0)));
    }
    out
}

impl Rigged {
    pub fn part_depth(&self, palette: &[Mat], part: usize) -> f32 {
        let list = self.stack.get(part).map_or(&[][..], Vec::as_slice);
        let z = |(j, s): &(u16, [f32; 4])| {
            palette.get(*j as usize).map_or(0.0, |m| m[8] * s[0] + m[9] * s[1] + m[10] * s[2] + m[11] * s[3])
        };
        list.iter().map(z).sum()
    }
}
