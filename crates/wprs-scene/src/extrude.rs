use crate::kin::Place;
use crate::puppet::limb::boxed;
use crate::resolve::Context;
use crate::{Affine, Lean, MeshDepth};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Relief {
    pub lift: [f32; 3],
    pub cull: bool,
}

fn times(m: Lean, v: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|r| (0..3).map(|c| m.0[r * 3 + c] * v[c]).sum())
}

fn column(m: Lean, c: usize) -> [f32; 3] {
    [m.0[c], m.0[3 + c], m.0[6 + c]]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

impl Context<'_> {
    pub(crate) fn relief(&self, place: &Place, (origin, size): ([f32; 2], [f32; 2]), d: MeshDepth) -> Relief {
        let (far, [px, py]) = (self.distance(), self.canvas.map(|v| v / 2.0));
        let corner = Affine::translate([-origin[0] * size[0], -origin[1] * size[1]]);
        let mesh = Lean::of(place.world.then(corner).then(boxed(size)));
        let full = place.lean.map_or(mesh, |l| l.then(mesh));
        let bare = Lean::of(self.camera.inverse()).then(full);
        let back = Lean([1.0, 0.0, -px, 0.0, 1.0, -py, 0.0, 0.0, far]);
        let [u, v] = [0, 1].map(|c| times(back, column(bare, c)));
        let n = cross(u, v);
        let area = n.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-12);
        let side = if u[0] * v[1] - u[1] * v[0] < 0.0 { 1.0 } else { -1.0 };
        let reach = d.scale.clamp(0.0, 10.0) * side / area.sqrt();
        let [dx, dy, dz] = n.map(|x| x * reach);
        let k = 1.0 / far;
        let seen = times(Lean::of(self.camera), [dx + px * dz * k, dy + py * dz * k, dz * k]);
        let lift = times(full.inverse(), seen);
        Relief { lift: if lift.iter().all(|v| v.is_finite()) { lift } else { [0.0; 3] }, cull: d.cull }
    }
}
