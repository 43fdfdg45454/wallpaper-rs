#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Affine(pub [f32; 6]);

impl Affine {
    pub const IDENTITY: Affine = Affine([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);

    pub fn then(self, inner: Affine) -> Affine {
        let ([a, b, c, d, e, f], [g, h, i, j, k, l]) = (self.0, inner.0);
        Affine([
            a * g + c * h,
            b * g + d * h,
            a * i + c * j,
            b * i + d * j,
            a * k + c * l + e,
            b * k + d * l + f,
        ])
    }

    pub fn translate([x, y]: [f32; 2]) -> Affine {
        Affine([1.0, 0.0, 0.0, 1.0, x, y])
    }

    pub fn scale([x, y]: [f32; 2]) -> Affine {
        Affine([x, 0.0, 0.0, y, 0.0, 0.0])
    }

    pub fn rotate(degrees: f32) -> Affine {
        let (s, c) = degrees.to_radians().sin_cos();
        Affine([c, s, -s, c, 0.0, 0.0])
    }

    pub fn skew([x, y]: [f32; 2]) -> Affine {
        Affine([1.0, y.to_radians().tan(), x.to_radians().tan(), 1.0, 0.0, 0.0])
    }

    pub fn apply(self, [x, y]: [f32; 2]) -> [f32; 2] {
        let [a, b, c, d, e, f] = self.0;
        [a * x + c * y + e, b * x + d * y + f]
    }
}
