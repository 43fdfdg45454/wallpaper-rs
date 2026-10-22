use super::parse::{invalid, usage};
use super::{Edit, Error};
use serde::{Deserialize, Serialize};

pub const MODEL: &str = "depth-anything-v2-small";
pub const STEPS: [u32; 4] = [1, 8, 16, 32];
pub const MAX_STRENGTH: f32 = 0.03;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Depth {
    pub model: String,
    pub input: u32,
    pub near: f32,
    pub far: f32,
    pub gamma: f32,
    pub smooth: u32,
    pub dilate: u32,
}

impl Default for Depth {
    fn default() -> Self {
        Self { model: MODEL.into(), input: 518, near: 1.0, far: 0.0, gamma: 1.0, smooth: 2, dilate: 3 }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Motion {
    #[default]
    Auto,
    Pointer,
    Accelerometer,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Parallax {
    pub enabled: bool,
    pub source: Motion,
    pub strength: f32,
    pub focus: f32,
    pub steps: u32,
    pub smoothing_ms: u32,
    pub overscan: f32,
}

impl Default for Parallax {
    fn default() -> Self {
        let (strength, focus, steps, smoothing_ms, overscan) = (0.02, 0.3, 16, 150, 1.04);
        Self { enabled: true, source: Motion::Auto, strength, focus, steps, smoothing_ms, overscan }
    }
}

pub(super) fn check(depth: Option<&Depth>, parallax: Option<&Parallax>) -> Result<(), String> {
    let within = |path: &str, value: f32, lo: f32, hi: f32| {
        let fine = (lo..=hi).contains(&value);
        fine.then_some(()).ok_or_else(|| format!("{path}: {value} is out of range {lo} to {hi}"))
    };
    if let Some(d) = depth {
        (d.model == MODEL).then_some(()).ok_or_else(|| format!("depth.model: use {MODEL}"))?;
        let input = (196..=1036).contains(&d.input) && d.input.is_multiple_of(14);
        input.then_some(()).ok_or("depth.input: use a multiple of 14 from 196 to 1036")?;
        within("depth.near", d.near, 0.0, 1.0)?;
        within("depth.far", d.far, 0.0, 1.0)?;
        within("depth.gamma", d.gamma, 0.2, 5.0)?;
        (d.smooth <= 16 && d.dilate <= 16).then_some(()).ok_or("depth: use 0 to 16 pixels")?;
    }
    let Some(p) = parallax else { return Ok(()) };
    within("parallax.strength", p.strength, 0.0, MAX_STRENGTH)?;
    within("parallax.focus", p.focus, 0.0, 1.0)?;
    within("parallax.overscan", p.overscan, 1.0, 1.1)?;
    STEPS.contains(&p.steps).then_some(()).ok_or("parallax.steps: use 1, 8, 16 or 32")?;
    (p.smoothing_ms <= 2000).then_some(()).ok_or_else(|| "parallax.smoothing_ms: use 0 to 2000".into())
}

pub(super) struct Look {
    pub(super) source: Option<String>,
    pub(super) strength: Option<f32>,
    pub(super) focus: Option<f32>,
    pub(super) steps: Option<u32>,
    pub(super) smoothing: Option<u32>,
    pub(super) overscan: Option<f32>,
    pub(super) off: bool,
}

impl Edit {
    pub(super) fn depth(&mut self, levels: [Option<f32>; 3], pixels: [Option<u32>; 3]) {
        let d = self.depth.get_or_insert_with(Depth::default);
        let [near, far, gamma] = levels;
        let [smooth, dilate, input] = pixels;
        (d.near, d.far, d.gamma) = (near.unwrap_or(d.near), far.unwrap_or(d.far), gamma.unwrap_or(d.gamma));
        (d.smooth, d.dilate, d.input) =
            (smooth.unwrap_or(d.smooth), dilate.unwrap_or(d.dilate), input.unwrap_or(d.input));
    }

    pub(super) fn parallax(&mut self, look: Look) -> Result<(), Error> {
        let source = look.source.map(|s| {
            let named = serde_json::from_value(serde_json::Value::String(s.clone()));
            named.map_err(|_| usage(format!("{s} is not auto, pointer or accelerometer")))
        });
        let source = source.transpose()?;
        let p = self.parallax.get_or_insert_with(Parallax::default);
        (p.enabled, p.source) = (!look.off, source.unwrap_or(p.source));
        (p.strength, p.focus) = (look.strength.unwrap_or(p.strength), look.focus.unwrap_or(p.focus));
        (p.steps, p.overscan) = (look.steps.unwrap_or(p.steps), look.overscan.unwrap_or(p.overscan));
        p.smoothing_ms = look.smoothing.unwrap_or(p.smoothing_ms);
        STEPS.contains(&p.steps).then_some(()).ok_or_else(|| invalid("use 1, 8, 16 or 32 steps"))
    }

    pub fn moving(&self) -> Option<&Parallax> {
        self.parallax.as_ref().filter(|p| p.enabled && p.strength > 0.0)
    }

    pub fn needs_depth(&self) -> bool {
        let deep = |name: &String| matches!(self.masks.get(name), Some(super::Mask::Depth { .. }));
        self.moving().is_some() || self.ops.iter().any(|op| op.enabled && op.mask.as_ref().is_some_and(deep))
    }
}
