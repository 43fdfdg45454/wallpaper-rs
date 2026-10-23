use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

pub(crate) const FAMILIES: [&str; 10] =
    ["sine", "quad", "cubic", "quart", "quint", "expo", "circ", "back", "elastic", "bounce"];

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(try_from = "Repr", into = "Repr")]
pub enum Ease {
    #[default]
    Linear,
    Step,
    Hold,
    In(u8),
    Out(u8),
    InOut(u8),
    Bezier([f32; 4]),
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum Repr {
    Name(String),
    Bezier([f32; 4]),
}

impl TryFrom<Repr> for Ease {
    type Error = String;

    fn try_from(repr: Repr) -> Result<Self, String> {
        let name = match repr {
            Repr::Bezier(b) => return Ok(Ease::Bezier(b)),
            Repr::Name(name) => name,
        };
        let family = |f: &str| FAMILIES.iter().position(|n| *n == f).map(|i| i as u8);
        let found = match name.as_str() {
            "linear" => Some(Ease::Linear),
            "step" => Some(Ease::Step),
            "hold" => Some(Ease::Hold),
            n => n
                .strip_prefix("in_out_")
                .and_then(family)
                .map(Ease::InOut)
                .or_else(|| n.strip_prefix("in_").and_then(family).map(Ease::In))
                .or_else(|| n.strip_prefix("out_").and_then(family).map(Ease::Out)),
        };
        found.ok_or_else(|| format!("unknown easing {name:?}"))
    }
}

impl From<Ease> for Repr {
    fn from(ease: Ease) -> Repr {
        let name = |prefix: &str, f: u8| Repr::Name(format!("{prefix}{}", FAMILIES[f as usize]));
        match ease {
            Ease::Linear => Repr::Name("linear".into()),
            Ease::Step => Repr::Name("step".into()),
            Ease::Hold => Repr::Name("hold".into()),
            Ease::In(f) => name("in_", f),
            Ease::Out(f) => name("out_", f),
            Ease::InOut(f) => name("in_out_", f),
            Ease::Bezier(b) => Repr::Bezier(b),
        }
    }
}

fn ease_in(family: u8, f: f32) -> f32 {
    let (c1, c4) = (1.70158, 2.0 * PI / 3.0);
    match family {
        0 => 1.0 - (f * PI / 2.0).cos(),
        1..=4 => f.powi(i32::from(family) + 1),
        5 if f <= 0.0 => 0.0,
        5 => 2f32.powf(10.0 * f - 10.0),
        6 => 1.0 - (1.0 - f * f).max(0.0).sqrt(),
        7 => (c1 + 1.0) * f * f * f - c1 * f * f,
        8 if f <= 0.0 || f >= 1.0 => f,
        8 => -(2f32.powf(10.0 * f - 10.0)) * ((10.0 * f - 10.75) * c4).sin(),
        _ => 1.0 - bounce_out(1.0 - f),
    }
}

fn bounce_out(f: f32) -> f32 {
    let (n, d) = (7.5625, 2.75);
    match f {
        f if f < 1.0 / d => n * f * f,
        f if f < 2.0 / d => n * (f - 1.5 / d).powi(2) + 0.75,
        f if f < 2.5 / d => n * (f - 2.25 / d).powi(2) + 0.9375,
        f => n * (f - 2.625 / d).powi(2) + 0.984375,
    }
}

fn bezier([x1, y1, x2, y2]: [f32; 4], x: f32) -> f32 {
    let curve =
        |a: f32, b: f32, s: f32| 3.0 * a * s * (1.0 - s).powi(2) + 3.0 * b * s * s * (1.0 - s) + s.powi(3);
    let slope = |a: f32, b: f32, s: f32| {
        3.0 * a * (1.0 - s).powi(2) + 6.0 * (b - a) * s * (1.0 - s) + 3.0 * (1.0 - b) * s * s
    };
    let mut s = x;
    for _ in 0..8 {
        let (err, d) = (curve(x1, x2, s) - x, slope(x1, x2, s));
        if err.abs() < 1e-6 || d.abs() < 1e-6 {
            break;
        }
        s = (s - err / d).clamp(0.0, 1.0);
    }
    if (curve(x1, x2, s) - x).abs() > 1e-4 {
        let (mut lo, mut hi) = (0.0, 1.0);
        for _ in 0..32 {
            s = (lo + hi) / 2.0;
            if curve(x1, x2, s) < x { lo = s } else { hi = s }
        }
    }
    curve(y1, y2, s)
}

impl Ease {
    pub fn apply(self, f: f32) -> f32 {
        let f = f.clamp(0.0, 1.0);
        match self {
            Ease::Linear => f,
            Ease::Step => f32::from(f > 0.0),
            Ease::Hold => f32::from(f >= 1.0),
            Ease::In(family) => ease_in(family, f),
            Ease::Out(family) => 1.0 - ease_in(family, 1.0 - f),
            Ease::InOut(family) if f < 0.5 => ease_in(family, 2.0 * f) / 2.0,
            Ease::InOut(family) => 1.0 - ease_in(family, 2.0 - 2.0 * f) / 2.0,
            Ease::Bezier(b) => bezier(b, f),
        }
    }
}
