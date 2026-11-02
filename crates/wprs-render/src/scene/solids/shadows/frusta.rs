use super::super::uniform::Light3;
use wprs_scene::{Beam, Eye, Frame, Lamp, Mat4, Stage3d, apply4, mul4};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Plan {
    pub(crate) lamp: usize,
    pub(crate) side: u32,
    pub(crate) layers: u32,
    pub(crate) mats: [Mat4; 6],
    pub(crate) at: [f32; 3],
    pub(crate) texel: f32,
    pub(crate) bias: f32,
}

pub(crate) fn perspective(fov: f32, near: f32, far: f32) -> Mat4 {
    let t = 1.0 / (fov.to_radians() * 0.5).tan();
    let k = far / (near - far);
    [t, 0.0, 0.0, 0.0, 0.0, t, 0.0, 0.0, 0.0, 0.0, k, -1.0, 0.0, 0.0, near * k, 0.0]
}

pub(crate) fn ortho(r: f32, near: f32, far: f32) -> Mat4 {
    let d = 1.0 / (far - near);
    [1.0 / r, 0.0, 0.0, 0.0, 0.0, 1.0 / r, 0.0, 0.0, 0.0, 0.0, -d, 0.0, 0.0, 0.0, -near * d, 1.0]
}

fn look(at: [f32; 3], dir: [f32; 3], up: [f32; 3]) -> Mat4 {
    let center = std::array::from_fn(|i| at[i] + dir[i]);
    Eye { at, center, up, fov: 90.0, near: 0.1, far: 1.0, aspect: 1.0 }.view()
}

fn upward(d: [f32; 3]) -> [f32; 3] {
    if d[1].abs() > 0.9 { [0.0, 0.0, 1.0] } else { [0.0, 1.0, 0.0] }
}

pub(crate) fn reach(frame: &Frame, stage: &Stage3d) -> ([f32; 3], f32) {
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for p in &frame.placed[stage.models.clone()] {
        let [a, b] = p.model.bounds;
        for k in 0..8 {
            let c = [
                if k & 1 == 0 { a[0] } else { b[0] },
                if k & 2 == 0 { a[1] } else { b[1] },
                if k & 4 == 0 { a[2] } else { b[2] },
            ];
            let w = apply4(&p.matrix, c, 1.0);
            (0..3).for_each(|i| (lo[i], hi[i]) = (lo[i].min(w[i]), hi[i].max(w[i])));
        }
    }
    if lo[0] > hi[0] {
        return ([0.0; 3], 1.0);
    }
    let mid = std::array::from_fn(|i| (lo[i] + hi[i]) * 0.5);
    let r = (0..3).map(|i| (hi[i] - lo[i]).powi(2)).sum::<f32>().sqrt() * 0.5;
    (mid, r.max(1e-3) * 1.1)
}

pub(crate) fn plan_for(
    frame: &Frame,
    stage: &Stage3d,
    (lamp, l, light): (usize, &Lamp, &Light3),
    side: u32,
) -> Plan {
    let (mid, r) = reach(frame, stage);
    let at = [light.at[0], light.at[1], light.at[2]];
    let dir = [light.dir[0], light.dir[1], light.dir[2]];
    let gap = (0..3).map(|i| (mid[i] - at[i]).powi(2)).sum::<f32>().sqrt();
    let far = light.at[3].max(1e-3).min(gap + r);
    let near = (gap - r).max(far * 0.002).max(1e-4);
    let bias = l.shadow.map_or(wprs_scene::SHADOW_BIAS, |s| s.1);
    let mut mats = [[0.0; 16]; 6];
    let (layers, texel) = match l.beam {
        Beam::Directional => {
            let from = std::array::from_fn(|i| mid[i] - dir[i] * r * 2.0);
            mats[0] = mul4(&ortho(r, 0.0, r * 4.0), &look(from, dir, upward(dir)));
            (1, 2.0 * r / side as f32)
        }
        Beam::Spot => {
            let outer = light.dir[3].clamp(-1.0, 1.0).acos().to_degrees() * 2.0;
            mats[0] =
                mul4(&perspective((outer + 4.0).clamp(10.0, 170.0), near, far), &look(at, dir, upward(dir)));
            (1, far / side as f32)
        }
        _ => {
            let faces: [([f32; 3], [f32; 3]); 6] = [
                ([1.0, 0.0, 0.0], [0.0, -1.0, 0.0]),
                ([-1.0, 0.0, 0.0], [0.0, -1.0, 0.0]),
                ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
                ([0.0, -1.0, 0.0], [0.0, 0.0, -1.0]),
                ([0.0, 0.0, 1.0], [0.0, -1.0, 0.0]),
                ([0.0, 0.0, -1.0], [0.0, -1.0, 0.0]),
            ];
            let p = perspective(90.0, near.min(far * 0.5), far);
            for (m, (d, u)) in mats.iter_mut().zip(faces) {
                *m = mul4(&p, &look(at, d, u));
            }
            (6, far / side as f32)
        }
    };
    Plan { lamp, side, layers, mats, at, texel, bias }
}
