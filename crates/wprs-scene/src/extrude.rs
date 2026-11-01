use crate::kin::Place;
use crate::puppet::limb::boxed;
use crate::resolve::Context;
use crate::{Affine, Layer, Lean, MeshDepth};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Relief {
    pub lift: [f32; 3],
    pub cull: bool,
}

fn times(m: Lean, v: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|r| (0..3).map(|c| m.0[r * 3 + c] * v[c]).sum())
}

fn det(a: Affine) -> f32 {
    (a.0[0] * a.0[3] - a.0[1] * a.0[2]).abs()
}

impl Context<'_> {
    pub(crate) fn relief(
        &self,
        layer: &Layer,
        place: &Place,
        (origin, size): ([f32; 2], [f32; 2]),
        d: MeshDepth,
    ) -> Relief {
        let tilt =
            layer.transform.tilt.as_ref().map_or([0.0; 2], |v| v.get(self).0.map(|a| a.clamp(-89.0, 89.0)));
        let [(sx, cx), (sy, cy)] = tilt.map(|a| a.to_radians().sin_cos());
        let n = [sy * cx, -sx, cx * cy];
        let (k, [px, py]) = (1.0 / self.distance(), self.canvas.map(|v| v / 2.0));
        let column = times(Lean::of(self.camera), [n[0] - px * n[2] * k, n[1] - py * n[2] * k, -n[2] * k]);
        let corner = Affine::translate([-origin[0] * size[0], -origin[1] * size[1]]);
        let mesh = Lean::of(place.world.then(corner).then(boxed(size)));
        let full = place.lean.map_or(mesh, |l| l.then(mesh));
        let scale = d.scale.clamp(0.0, 10.0) * (det(place.world) / det(self.camera).max(1e-12)).sqrt();
        let lift = times(full.inverse(), column).map(|v| v * scale);
        Relief { lift: if lift.iter().all(|v| v.is_finite()) { lift } else { [0.0; 3] }, cull: d.cull }
    }
}
