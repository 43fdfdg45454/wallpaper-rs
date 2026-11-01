use super::Filter;
use super::looks::{NEUTRAL, Recipe};

type Mat = [[f32; 3]; 3];

const LUMA: [f32; 3] = [0.2126, 0.7152, 0.0722];
const SEPIA: Mat = [[0.393, 0.769, 0.189], [0.349, 0.686, 0.168], [0.272, 0.534, 0.131]];
const GAMMA: f32 = 2.2;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Grade {
    pub rows: [[f32; 4]; 3],
    pub lift: [f32; 4],
    pub gain: [f32; 4],
}

fn mul(a: Mat, b: Mat) -> Mat {
    std::array::from_fn(|r| std::array::from_fn(|c| (0..3).map(|k| a[r][k] * b[k][c]).sum()))
}

fn diag(d: [f32; 3]) -> Mat {
    std::array::from_fn(|r| std::array::from_fn(|c| if r == c { d[r] } else { 0.0 }))
}

fn saturate(s: f32) -> Mat {
    std::array::from_fn(|r| std::array::from_fn(|c| (1.0 - s) * LUMA[c] + if r == c { s } else { 0.0 }))
}

fn turn(degrees: f32) -> Mat {
    let (sin, cos) = degrees.to_radians().sin_cos();
    let [l, m, n] = [0.213, 0.715, 0.072];
    [
        [l + cos * (1.0 - l) - sin * l, m - cos * m - sin * m, n - cos * n + sin * (1.0 - n)],
        [l - cos * l + sin * 0.143, m + cos * (1.0 - m) + sin * 0.140, n - cos * n - sin * 0.283],
        [l - cos * l - sin * (1.0 - l), m - cos * m + sin * m, n + cos * (1.0 - n) + sin * n],
    ]
}

fn lerp(a: f32, b: f32, k: f32) -> f32 {
    a + (b - a) * k
}

fn blend(r: Recipe, k: f32) -> Recipe {
    let n = NEUTRAL;
    Recipe {
        exposure: lerp(n.exposure, r.exposure, k),
        contrast: lerp(n.contrast, r.contrast, k),
        saturation: lerp(n.saturation, r.saturation, k),
        warmth: lerp(n.warmth, r.warmth, k),
        tint: lerp(n.tint, r.tint, k),
        sepia: lerp(n.sepia, r.sepia, k),
        lift: std::array::from_fn(|i| lerp(n.lift[i], r.lift[i], k)),
        gain: std::array::from_fn(|i| lerp(n.gain[i], r.gain[i], k)),
    }
}

impl Grade {
    pub(super) fn of(f: &Filter) -> Grade {
        let r = blend(f.look.recipe(), f.strength.clamp(0.0, 1.0));
        let warmth = (r.warmth + f.warmth).clamp(-2.0, 2.0);
        let exposure = 2f32.powf((r.exposure + f.exposure).clamp(-4.0, 4.0));
        let gains = [1.0 + 0.12 * warmth, 1.0 - 0.1 * r.tint, 1.0 - 0.12 * warmth].map(|g| g * exposure);
        let sepia: Mat = std::array::from_fn(|i| {
            std::array::from_fn(|j| lerp(if i == j { 1.0 } else { 0.0 }, SEPIA[i][j], r.sepia))
        });
        let saturation = (r.saturation * (1.0 + f.saturation)).max(0.0);
        let m = mul(sepia, mul(turn(f.hue), mul(saturate(saturation), diag(gains))));
        let contrast = (r.contrast * (1.0 + f.contrast)).max(0.0);
        let [lr, lg, lb] = r.lift;
        let [gr, gg, gb] = r.gain;
        Grade {
            rows: m.map(|[a, b, c]| [a, b, c, 0.0]),
            lift: [lr, lg, lb, contrast],
            gain: [gr, gg, gb, 1.0],
        }
    }

    pub fn uniform(&self) -> [[f32; 4]; 5] {
        [self.rows[0], self.rows[1], self.rows[2], self.lift, self.gain]
    }

    pub fn apply(&self, c: [f32; 3]) -> [f32; 3] {
        let mixed = self.rows.map(|[a, b, d, o]| a * c[0] + b * c[1] + d * c[2] + o);
        std::array::from_fn(|i| {
            let p = mixed[i].max(0.0).powf(1.0 / GAMMA);
            let p = (p - 0.5) * self.lift[3] + 0.5;
            let p = self.lift[i] + p * (self.gain[i] - self.lift[i]);
            p.clamp(0.0, 1.0).powf(GAMMA)
        })
    }
}
