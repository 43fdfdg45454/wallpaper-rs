use crate::Affine;

pub const MIN_DEPTH: f32 = 1e-3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lean(pub [f32; 9]);

impl Lean {
    pub const IDENTITY: Lean = Lean([1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]);

    pub fn of(m: Affine) -> Lean {
        let [a, b, c, d, e, f] = m.0;
        Lean([a, c, e, b, d, f, 0.0, 0.0, 1.0])
    }

    pub fn then(self, inner: Lean) -> Lean {
        let (l, r) = (self.0, inner.0);
        Lean(std::array::from_fn(|k| {
            let (row, col) = (k / 3, k % 3);
            (0..3).map(|j| l[row * 3 + j] * r[j * 3 + col]).sum()
        }))
    }

    pub fn depth(self, [x, y]: [f32; 2]) -> f32 {
        let m = self.0;
        m[6] * x + m[7] * y + m[8]
    }

    pub fn apply(self, [x, y]: [f32; 2]) -> [f32; 2] {
        let m = self.0;
        let w = self.depth([x, y]).max(MIN_DEPTH);
        [(m[0] * x + m[1] * y + m[2]) / w, (m[3] * x + m[4] * y + m[5]) / w]
    }

    pub fn inverse(self) -> Lean {
        let [a, b, c, d, e, f, g, h, i] = self.0;
        let co = [e * i - f * h, c * h - b * i, b * f - c * e];
        let det = a * co[0] + d * co[1] + g * co[2];
        if det.abs() < 1e-12 {
            return Lean([0.0; 9]);
        }
        let rest = [f * g - d * i, a * i - c * g, c * d - a * f, d * h - e * g, b * g - a * h, a * e - b * d];
        Lean([co[0], co[1], co[2], rest[0], rest[1], rest[2], rest[3], rest[4], rest[5]].map(|v| v / det))
    }

    pub fn within(self, pivot: [f32; 2], tilt: [f32; 2], z: f32, (center, d): ([f32; 2], f32)) -> Lean {
        let (t, [px, py], [ox, oy]) = (self.0, center, pivot);
        let a3: [f32; 3] = std::array::from_fn(|i| d * (f32::from(u8::from(i == 2)) - t[6 + i]));
        let a1: [f32; 3] = std::array::from_fn(|i| t[i] + px / d * a3[i]);
        let a2: [f32; 3] = std::array::from_fn(|i| t[3 + i] + py / d * a3[i]);
        let col = |i: usize| [a1[i], a2[i], a3[i]];
        let (m1, m2, base) = (col(0), col(1), col(2));
        let c = crate::eye::cross(m1, m2);
        let n = c.map(|v| v / c.iter().map(|v| v * v).sum::<f32>().sqrt().sqrt().max(1e-12));
        let [(sx, cx), (sy, cy)] = tilt.map(|t| t.to_radians().sin_cos());
        let (rx, ry) = ([cy, 0.0, -sy], [sx * sy, cx, sx * cy]);
        let p0: [f32; 3] = std::array::from_fn(|i| [ox, oy, z][i] - rx[i] * ox - ry[i] * oy);
        let outer = |p: [f32; 3], one: f32| -> [f32; 3] {
            std::array::from_fn(|i| m1[i] * p[0] + m2[i] * p[1] + n[i] * p[2] + base[i] * one)
        };
        let (u, v, w) = (outer(rx, 0.0), outer(ry, 0.0), outer(p0, 1.0));
        let row = |k: usize, c: f32| [u[k] - c / d * u[2], v[k] - c / d * v[2], w[k] - c / d * w[2]];
        let ([r0, r1, r2], [r3, r4, r5]) = (row(0, px), row(1, py));
        Lean([r0, r1, r2, r3, r4, r5, -u[2] / d, -v[2] / d, 1.0 - w[2] / d])
    }

    pub fn tilted(pivot: [f32; 2], tilt: [f32; 2], z: f32, (center, distance): ([f32; 2], f32)) -> Lean {
        let [(sx, cx), (sy, cy)] = tilt.map(|t| t.to_radians().sin_cos());
        let (k, [ox, oy], [px, py]) = (1.0 / distance, pivot, center);
        let lean = Lean([
            cy + px * sy * k,
            sx * sy - px * sx * cy * k,
            ox - px * z * k,
            py * sy * k,
            cx - py * sx * cy * k,
            oy - py * z * k,
            sy * k,
            -sx * cy * k,
            1.0 - z * k,
        ]);
        lean.then(Lean::of(Affine::translate([-ox, -oy])))
    }
}
