pub type Mat = [f32; 12];
pub type Quat = [f32; 4];

pub const IDENTITY: Mat = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Trs {
    pub t: [f32; 3],
    pub r: Quat,
    pub s: [f32; 3],
}

impl Default for Trs {
    fn default() -> Self {
        Trs { t: [0.0; 3], r: [0.0, 0.0, 0.0, 1.0], s: [1.0; 3] }
    }
}

impl Trs {
    pub fn matrix(&self) -> Mat {
        let [x, y, z, w] = self.r;
        let [sx, sy, sz] = self.s;
        let [tx, ty, tz] = self.t;
        let (xx, yy, zz, xy, xz, yz, wx, wy, wz) =
            (x * x, y * y, z * z, x * y, x * z, y * z, w * x, w * y, w * z);
        [
            (1.0 - 2.0 * (yy + zz)) * sx,
            2.0 * (xy - wz) * sy,
            2.0 * (xz + wy) * sz,
            tx,
            2.0 * (xy + wz) * sx,
            (1.0 - 2.0 * (xx + zz)) * sy,
            2.0 * (yz - wx) * sz,
            ty,
            2.0 * (xz - wy) * sx,
            2.0 * (yz + wx) * sy,
            (1.0 - 2.0 * (xx + yy)) * sz,
            tz,
        ]
    }

    pub fn lerp(&self, to: &Trs, f: f32) -> Trs {
        let mix = |a: [f32; 3], b: [f32; 3]| std::array::from_fn(|i| a[i] + (b[i] - a[i]) * f);
        Trs { t: mix(self.t, to.t), r: nlerp(self.r, to.r, f), s: mix(self.s, to.s) }
    }
}

pub fn mul(a: &Mat, b: &Mat) -> Mat {
    let mut out = [0.0; 12];
    for r in 0..3 {
        for c in 0..4 {
            let v = (0..3).map(|k| a[r * 4 + k] * b[k * 4 + c]).sum::<f32>();
            out[r * 4 + c] = v + if c == 3 { a[r * 4 + 3] } else { 0.0 };
        }
    }
    out
}

pub fn apply(m: &Mat, [x, y, z]: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|r| m[r * 4] * x + m[r * 4 + 1] * y + m[r * 4 + 2] * z + m[r * 4 + 3])
}

pub fn invert(m: &Mat) -> Mat {
    let a = |r: usize, c: usize| m[r * 4 + c];
    let cof = |r: usize, c: usize| {
        let (r0, r1, c0, c1) = ((r + 1) % 3, (r + 2) % 3, (c + 1) % 3, (c + 2) % 3);
        a(r0, c0) * a(r1, c1) - a(r0, c1) * a(r1, c0)
    };
    let det = (0..3).map(|c| a(0, c) * cof(0, c)).sum::<f32>();
    if det.abs() < 1e-20 {
        return IDENTITY;
    }
    let mut out = [0.0; 12];
    for r in 0..3 {
        for c in 0..3 {
            out[r * 4 + c] = cof(c, r) / det;
        }
    }
    for r in 0..3 {
        out[r * 4 + 3] = -(0..3).map(|k| out[r * 4 + k] * a(k, 3)).sum::<f32>();
    }
    out
}

pub fn decompose(m: &Mat) -> Trs {
    let col = |c: usize| [m[c], m[4 + c], m[8 + c]];
    let len = |v: [f32; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let (x, y, z) = (col(0), col(1), col(2));
    let det = x[0] * (y[1] * z[2] - z[1] * y[2]) - y[0] * (x[1] * z[2] - z[1] * x[2])
        + z[0] * (x[1] * y[2] - y[1] * x[2]);
    let s = [len(x) * if det < 0.0 { -1.0 } else { 1.0 }, len(y), len(z)];
    let unit = |v: [f32; 3], l: f32| v.map(|e| if l.abs() > 1e-12 { e / l } else { 0.0 });
    let (x, y, z) = (unit(x, s[0]), unit(y, s[1]), unit(z, s[2]));
    Trs { t: [m[3], m[7], m[11]], r: from_basis(x, y, z), s }
}

fn from_basis(x: [f32; 3], y: [f32; 3], z: [f32; 3]) -> Quat {
    let trace = x[0] + y[1] + z[2];
    let q = if trace > 0.0 {
        let s = (trace + 1.0).sqrt() * 2.0;
        [(y[2] - z[1]) / s, (z[0] - x[2]) / s, (x[1] - y[0]) / s, s / 4.0]
    } else if x[0] > y[1] && x[0] > z[2] {
        let s = (1.0 + x[0] - y[1] - z[2]).sqrt() * 2.0;
        [s / 4.0, (y[0] + x[1]) / s, (z[0] + x[2]) / s, (y[2] - z[1]) / s]
    } else if y[1] > z[2] {
        let s = (1.0 + y[1] - x[0] - z[2]).sqrt() * 2.0;
        [(y[0] + x[1]) / s, s / 4.0, (z[1] + y[2]) / s, (z[0] - x[2]) / s]
    } else {
        let s = (1.0 + z[2] - x[0] - y[1]).sqrt() * 2.0;
        [(z[0] + x[2]) / s, (z[1] + y[2]) / s, s / 4.0, (x[1] - y[0]) / s]
    };
    normalized(q)
}

pub fn normalized(q: Quat) -> Quat {
    let n = q.iter().map(|v| v * v).sum::<f32>().sqrt();
    if n > 1e-12 { q.map(|v| v / n) } else { [0.0, 0.0, 0.0, 1.0] }
}

pub fn nlerp(a: Quat, b: Quat, f: f32) -> Quat {
    let dot = (0..4).map(|i| a[i] * b[i]).sum::<f32>();
    let sign = if dot < 0.0 { -1.0 } else { 1.0 };
    normalized(std::array::from_fn(|i| a[i] + (b[i] * sign - a[i]) * f))
}

pub fn slerp(a: Quat, b: Quat, f: f32) -> Quat {
    let dot = (0..4).map(|i| a[i] * b[i]).sum::<f32>();
    let (b, dot) = if dot < 0.0 { (b.map(|v| -v), -dot) } else { (b, dot) };
    if dot > 0.9995 {
        return nlerp(a, b, f);
    }
    let theta = dot.min(1.0).acos();
    let (wa, wb) = (((1.0 - f) * theta).sin() / theta.sin(), (f * theta).sin() / theta.sin());
    normalized(std::array::from_fn(|i| a[i] * wa + b[i] * wb))
}

pub fn qmul(a: Quat, b: Quat) -> Quat {
    let ([ax, ay, az, aw], [bx, by, bz, bw]) = (a, b);
    [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]
}

pub fn conjugate([x, y, z, w]: Quat) -> Quat {
    [-x, -y, -z, w]
}

pub fn turn(clockwise: f32) -> Quat {
    let (s, c) = (-clockwise.to_radians() / 2.0).sin_cos();
    [0.0, 0.0, s, c]
}

pub fn beyond(rest: &Trs) -> [f32; 3] {
    let [x, y, z] = rest.t;
    let back = qmul(qmul(conjugate(rest.r), [x, y, z, 0.0]), rest.r);
    let t: [f32; 3] = std::array::from_fn(|i| back[i] / rest.s[i].abs().max(1e-6));
    if t[0].hypot(t[1]) < 1e-3 { [32.0, 0.0, 0.0] } else { t }
}
