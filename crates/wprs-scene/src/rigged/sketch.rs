use super::Rigged;
use super::math::{apply, mul};

pub const MAX_SKETCH_BONES: usize = 128;

#[derive(Debug, Clone, PartialEq)]
pub struct SketchBone {
    pub name: String,
    pub parent: Option<usize>,
    pub at: [f32; 2],
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Sketch {
    pub size: [f32; 2],
    pub points: Vec<[f32; 2]>,
    pub triangles: Vec<[u32; 3]>,
    pub bones: Vec<SketchBone>,
    pub weights: Vec<[(u16, f32); 4]>,
    pub shapes: Vec<(String, Vec<[f32; 2]>)>,
}

impl Sketch {
    pub fn mesh(&self, [x, y]: [f32; 2]) -> [f32; 3] {
        [x - self.size[0] / 2.0, self.size[1] / 2.0 - y, 0.0]
    }

    pub fn picture(&self, [x, y, _]: [f32; 3]) -> [f32; 2] {
        [x + self.size[0] / 2.0, self.size[1] / 2.0 - y]
    }

    pub fn bind(&mut self) {
        let heads: Vec<[f32; 2]> = self.bones.iter().map(|b| b.at).collect();
        let tail =
            |b: usize| self.bones.iter().position(|c| c.parent == Some(b)).map_or(heads[b], |c| heads[c]);
        let segments: Vec<([f32; 2], [f32; 2])> = (0..heads.len()).map(|b| (heads[b], tail(b))).collect();
        let band = (self.size[0].max(self.size[1]) * 0.04).max(1.0);
        self.weights = self
            .points
            .iter()
            .map(|p| {
                let gaps: Vec<f32> = segments.iter().map(|(a, z)| distance(*p, *a, *z)).collect();
                let least = gaps.iter().copied().fold(f32::MAX, f32::min);
                let mut near: Vec<(u16, f32)> = gaps
                    .iter()
                    .enumerate()
                    .map(|(b, d)| (b as u16, (-((d - least) / band).powi(2)).exp()))
                    .collect();
                near.sort_by(|x, y| y.1.total_cmp(&x.1));
                near.truncate(4);
                let sum: f32 = near.iter().map(|n| n.1).sum::<f32>().max(f32::MIN_POSITIVE);
                let mut out = [(0, 0.0); 4];
                near.iter().enumerate().for_each(|(k, n)| out[k] = (n.0, n.1 / sum));
                if near.is_empty() {
                    out[0].1 = 1.0;
                }
                out
            })
            .collect();
    }

    pub fn from_rig(rig: &Rigged, size: [f32; 2]) -> Option<Sketch> {
        let [piece] = &rig.pieces[..] else { return None };
        let mut sketch = Sketch { size, ..Sketch::default() };
        let skin: Vec<u16> = rig.palette.iter().map(|(n, _)| *n).filter(|n| *n != 0).collect();
        let worlds = worlds(rig);
        for node in &skin {
            let joint = &rig.joints[*node as usize];
            let parent = joint.parent.and_then(|p| skin.iter().position(|s| *s == p));
            let at = sketch.picture(apply(&worlds[*node as usize], [0.0; 3]));
            sketch.bones.push(SketchBone { name: joint.name.clone(), parent, at });
        }
        for v in rig.vertices.chunks(super::VERTEX_FLOATS) {
            sketch.points.push(sketch.picture([v[0], v[1], v[2]]));
            let w: [(u16, f32); 4] = std::array::from_fn(|k| (v[5 + k] as u16, v[9 + k]));
            sketch.weights.push(w);
        }
        for &(shape, base) in &piece.targets {
            let deltas = &rig.morphs[base as usize..(base + piece.vertices) as usize];
            sketch
                .shapes
                .push((rig.shapes[shape as usize].clone(), deltas.iter().map(|d| [d[0], -d[1]]).collect()));
        }
        let tris = rig.indices[piece.first as usize..(piece.first + piece.count) as usize].chunks(3);
        sketch.triangles = tris.map(|t| [t[0], t[1], t[2]]).collect();
        Some(sketch)
    }
}

pub(super) fn worlds(rig: &Rigged) -> Vec<super::math::Mat> {
    let mut out = vec![super::math::IDENTITY; rig.joints.len()];
    for &n in &rig.order {
        let j = &rig.joints[n as usize];
        let local = j.rest.matrix();
        out[n as usize] = j.parent.map_or(local, |p| mul(&out[p as usize], &local));
    }
    out
}

fn distance(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let (ab, ap) = ([b[0] - a[0], b[1] - a[1]], [p[0] - a[0], p[1] - a[1]]);
    let long = ab[0] * ab[0] + ab[1] * ab[1];
    let t = if long > 0.0 { ((ap[0] * ab[0] + ap[1] * ab[1]) / long).clamp(0.0, 1.0) } else { 0.0 };
    let d = [ap[0] - ab[0] * t, ap[1] - ab[1] * t];
    (d[0] * d[0] + d[1] * d[1]).sqrt()
}
