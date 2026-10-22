use crate::Color;
use serde::{Deserialize, Serialize};

pub const EFFECTS_WGSL: &str = include_str!("effects.wgsl");

pub(crate) type Param = (&'static str, f32, f32, f32);

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Effect {
    Blur { radius: f32 },
    ColorGrade { brightness: f32, contrast: f32, saturation: f32, hue: f32 },
    Waves { amplitude: f32, frequency: f32, speed: f32, direction: f32 },
    Scroll { speed_x: f32, speed_y: f32 },
    Shake { amplitude: f32, frequency: f32 },
    Vignette { strength: f32, radius: f32 },
    Pulse { amount: f32, speed: f32 },
    Tint { color: Color, amount: f32 },
    Pixelate { size: f32 },
    ChromaticAberration { amount: f32 },
}

impl Effect {
    pub fn name(&self) -> &'static str {
        self.spec().0
    }

    pub fn uniforms(&self) -> [f32; 8] {
        let mut out = [0.0; 8];
        self.spec().1.iter().zip(&mut out).for_each(|(p, u)| *u = p.1);
        out
    }

    pub fn shader(&self) -> String {
        let entry = "@fragment\nfn fs_main(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {";
        format!("{EFFECTS_WGSL}\n{entry}\n    return fx_{}(uv);\n}}\n", self.name())
    }

    pub(crate) fn spec(&self) -> (&'static str, Vec<Param>) {
        use Effect::*;
        match *self {
            Blur { radius } => ("blur", vec![("radius", radius, 0.0, 128.0)]),
            ColorGrade { brightness, contrast, saturation, hue } => (
                "color_grade",
                vec![
                    ("brightness", brightness, -1.0, 1.0),
                    ("contrast", contrast, 0.0, 4.0),
                    ("saturation", saturation, 0.0, 4.0),
                    ("hue", hue, -180.0, 180.0),
                ],
            ),
            Waves { amplitude, frequency, speed, direction } => (
                "waves",
                vec![
                    ("amplitude", amplitude, 0.0, 512.0),
                    ("frequency", frequency, 0.0, 100.0),
                    ("speed", speed, -100.0, 100.0),
                    ("direction", direction, -360.0, 360.0),
                ],
            ),
            Scroll { speed_x, speed_y } => {
                ("scroll", vec![("speed_x", speed_x, -1e4, 1e4), ("speed_y", speed_y, -1e4, 1e4)])
            }
            Shake { amplitude, frequency } => {
                ("shake", vec![("amplitude", amplitude, 0.0, 512.0), ("frequency", frequency, 0.0, 60.0)])
            }
            Vignette { strength, radius } => {
                ("vignette", vec![("strength", strength, 0.0, 1.0), ("radius", radius, 0.01, 2.0)])
            }
            Pulse { amount, speed } => {
                ("pulse", vec![("amount", amount, 0.0, 1.0), ("speed", speed, 0.0, 60.0)])
            }
            Tint { color: Color([r, g, b, a]), amount } => (
                "tint",
                vec![
                    ("color/0", r, 0.0, 1.0),
                    ("color/1", g, 0.0, 1.0),
                    ("color/2", b, 0.0, 1.0),
                    ("color/3", a, 0.0, 1.0),
                    ("amount", amount, 0.0, 1.0),
                ],
            ),
            Pixelate { size } => ("pixelate", vec![("size", size, 1.0, 1024.0)]),
            ChromaticAberration { amount } => ("chromatic_aberration", vec![("amount", amount, 0.0, 64.0)]),
        }
    }
}
