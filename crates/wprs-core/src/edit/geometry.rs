use super::{Edit, Geometry};

pub type Matrix = [[f64; 3]; 3];

const IDENTITY: Matrix = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

fn affine([a, b, c]: [f64; 3], [d, e, f]: [f64; 3]) -> Matrix {
    [[a, b, c], [d, e, f], [0.0, 0.0, 1.0]]
}

pub fn mul(a: &Matrix, b: &Matrix) -> Matrix {
    std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| a[i][k] * b[k][j]).sum()))
}

impl Geometry {
    pub fn region(&self) -> [f32; 4] {
        self.crop.unwrap_or([0.0, 0.0, 1.0, 1.0])
    }

    fn turned(&self, (w, h): (u32, u32)) -> (f64, f64) {
        let (w, h) = (f64::from(w.max(1)), f64::from(h.max(1)));
        if self.rotate % 180 == 90 { (h, w) } else { (w, h) }
    }

    fn inscribed(&self, (w, h): (f64, f64)) -> f64 {
        let (sin, cos) = f64::from(self.straighten).to_radians().abs().sin_cos();
        (w / (w * cos + h * sin)).min(h / (w * sin + h * cos))
    }

    pub fn frame(&self, image: (u32, u32)) -> (u32, u32) {
        let (w, h) = self.turned(image);
        let (s, [.., cw, ch]) = (self.inscribed((w, h)), self.region().map(f64::from));
        ((w * s * cw).round().max(1.0) as u32, (h * s * ch).round().max(1.0) as u32)
    }

    pub fn matrix(&self, image: (u32, u32)) -> Matrix {
        let (w, h) = self.turned(image);
        let (s, [cx, cy, cw, ch]) = (self.inscribed((w, h)), self.region().map(f64::from));
        let (sin, cos) = f64::from(self.straighten).to_radians().sin_cos();
        let [fx, fy] = self.flip.map(|f| if f { -1.0 } else { 1.0 });
        let unturn = match self.rotate % 360 {
            90 => affine([0.0, 1.0, 0.0], [-1.0, 0.0, 1.0]),
            180 => affine([-1.0, 0.0, 1.0], [0.0, -1.0, 1.0]),
            270 => affine([0.0, -1.0, 1.0], [1.0, 0.0, 0.0]),
            _ => IDENTITY,
        };
        let steps = [
            affine([cw, 0.0, cx], [0.0, ch, cy]),
            affine([w * s, 0.0, -0.5 * w * s], [0.0, h * s, -0.5 * h * s]),
            affine([cos, -sin, 0.0], [sin, cos, 0.0]),
            affine([fx, 0.0, 0.0], [0.0, fy, 0.0]),
            affine([1.0 / w, 0.0, 0.5], [0.0, 1.0 / h, 0.5]),
            unturn,
        ];
        steps.iter().fold(IDENTITY, |done, step| mul(step, &done))
    }

    pub fn is_identity(&self) -> bool {
        self.region() == [0.0, 0.0, 1.0, 1.0]
            && self.rotate.is_multiple_of(360)
            && self.straighten == 0.0
            && self.flip == [false; 2]
    }
}

impl Edit {
    pub fn develops(&self) -> bool {
        !self.geometry.is_identity() || self.ops.iter().any(|op| op.enabled)
    }

    pub fn crop_for(&self, output: &str, identity: Option<&str>) -> Option<[f32; 4]> {
        let named = identity.filter(|i| !i.is_empty()).and_then(|i| self.outputs.get(i));
        let found = named.or_else(|| self.outputs.get(output));
        found.or_else(|| self.outputs.values().find(|o| o.connector == output)).map(|o| o.crop)
    }
}
