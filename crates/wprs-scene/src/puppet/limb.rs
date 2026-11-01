use crate::Affine;
use crate::rigged::Rigged;
use crate::rigged::math::{Mat, Trs, conjugate, normalized, qmul};
use std::f32::consts::PI;

pub(crate) fn tip(rig: &Rigged, n: usize) -> [f32; 3] {
    rig.tip(n)
}

pub(crate) fn pivot(w: &Mat) -> [f32; 2] {
    [w[3], w[7]]
}

pub(crate) fn end(w: &Mat, [x, y, z]: [f32; 3]) -> [f32; 2] {
    [w[0] * x + w[1] * y + w[2] * z + w[3], w[4] * x + w[5] * y + w[6] * z + w[7]]
}

pub(crate) fn flat(w: &Mat) -> Affine {
    Affine([w[0], w[4], w[1], w[5], w[3], w[7]])
}

pub(crate) fn angle(v: [f32; 2]) -> f32 {
    v[1].atan2(v[0])
}

pub(crate) fn gap(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [b[0] - a[0], b[1] - a[1]]
}

pub(crate) fn length(v: [f32; 2]) -> f32 {
    v[0].hypot(v[1])
}

pub(crate) fn wrap(a: f32) -> f32 {
    let a = (a + PI).rem_euclid(2.0 * PI) - PI;
    if a <= -PI { a + 2.0 * PI } else { a }
}

pub(crate) fn around(p: [f32; 2], center: [f32; 2], a: f32) -> [f32; 2] {
    let (s, c) = a.sin_cos();
    let [x, y] = gap(center, p);
    [center[0] + c * x - s * y, center[1] + s * x + c * y]
}

pub(crate) fn mirrored(w: Option<&Mat>) -> f32 {
    match w {
        Some(w) if w[0] * w[5] - w[1] * w[4] < 0.0 => -1.0,
        _ => 1.0,
    }
}

pub(crate) fn turn(local: &mut Trs, ccw: f32) {
    let (s, c) = (ccw / 2.0).sin_cos();
    local.r = normalized(qmul([0.0, 0.0, s, c], local.r));
}

pub(crate) fn offset(local: &Trs, rest: &Trs) -> f32 {
    let q = qmul(conjugate(rest.r), local.r);
    wrap(2.0 * q[2].atan2(q[3]))
}

pub(crate) fn range(limit: Option<[f32; 2]>) -> Option<[f32; 2]> {
    limit.map(|[lo, hi]| [(-hi).to_radians(), (-lo).to_radians()])
}

pub(crate) fn boxed([w, h]: [f32; 2]) -> Affine {
    Affine([1.0, 0.0, 0.0, -1.0, w / 2.0, h / 2.0])
}

pub(crate) fn frame(w: &Mat, size: [f32; 2]) -> Affine {
    boxed(size).then(flat(w)).then(Affine::scale([1.0, -1.0]))
}
