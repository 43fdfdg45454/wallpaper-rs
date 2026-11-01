use serde::{Deserialize, Serialize};

mod grade;
mod looks;

pub use grade::Grade;
pub use looks::{LOOKS, Look};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Filter {
    pub look: Look,
    pub strength: f32,
    pub exposure: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub warmth: f32,
    pub hue: f32,
}

impl Default for Filter {
    fn default() -> Self {
        Filter {
            look: Look::None,
            strength: 1.0,
            exposure: 0.0,
            contrast: 0.0,
            saturation: 0.0,
            warmth: 0.0,
            hue: 0.0,
        }
    }
}

pub const RANGES: [(&str, f32, f32); 6] = [
    ("strength", 0.0, 1.0),
    ("exposure", -3.0, 3.0),
    ("contrast", -1.0, 1.0),
    ("saturation", -1.0, 1.0),
    ("warmth", -1.0, 1.0),
    ("hue", -180.0, 180.0),
];

impl Filter {
    fn fields(&mut self) -> [&mut f32; 6] {
        [
            &mut self.strength,
            &mut self.exposure,
            &mut self.contrast,
            &mut self.saturation,
            &mut self.warmth,
            &mut self.hue,
        ]
    }

    pub fn valid(&self) -> bool {
        let mut copy = *self;
        copy.fields().into_iter().zip(RANGES).all(|(v, (_, lo, hi))| v.is_finite() && (lo..=hi).contains(v))
    }

    pub fn sanitized(mut self) -> Filter {
        let neutral = Filter::default().fields().map(|v| *v);
        for ((v, (_, lo, hi)), n) in self.fields().into_iter().zip(RANGES).zip(neutral) {
            *v = if v.is_finite() { v.clamp(lo, hi) } else { n };
        }
        self
    }

    pub fn set(&mut self, name: &str, value: f32) -> Result<(), String> {
        let at = RANGES
            .iter()
            .position(|(n, ..)| *n == name)
            .ok_or_else(|| format!("unknown filter setting '{name}'"))?;
        let (_, lo, hi) = RANGES[at];
        if !value.is_finite() || !(lo..=hi).contains(&value) {
            return Err(format!("{name} takes {lo} to {hi}"));
        }
        *self.fields()[at] = value;
        Ok(())
    }

    pub fn is_identity(&self) -> bool {
        let plain = Filter { look: Look::None, strength: 1.0, ..*self };
        (self.look == Look::None || self.strength == 0.0)
            && plain == Filter { strength: 1.0, ..Filter::default() }
    }

    pub fn grade(&self) -> Option<Grade> {
        (!self.is_identity()).then(|| Grade::of(self))
    }
}
