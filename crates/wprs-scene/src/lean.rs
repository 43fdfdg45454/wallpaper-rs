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
