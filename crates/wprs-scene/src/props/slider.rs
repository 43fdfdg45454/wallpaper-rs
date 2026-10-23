use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::ops::Not;

const DECIMALS: u32 = 2;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Slider {
    pub min: f64,
    pub max: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<f64>,
    pub default: f64,
    #[serde(default)]
    pub integer: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-range" = [0, 6]))]
    pub decimals: Option<u8>,
    #[serde(default, skip_serializing_if = "Not::not")]
    pub editable: bool,
    #[serde(default, skip_serializing_if = "Not::not")]
    pub log: bool,
}

impl Slider {
    pub fn new(min: f64, max: f64, step: Option<f64>, default: f64) -> Slider {
        let (integer, editable, log, unit, decimals) = (false, false, false, None, None);
        Slider { min, max, step, default, integer, unit, decimals, editable, log }
    }

    pub fn places(&self) -> u32 {
        let from_step = |step: f64| (0..=6).find(|d| (step * 10f64.powi(*d)).fract().abs() < 1e-9);
        let found = self.decimals.map(u32::from).or_else(|| self.step.and_then(from_step).map(|d| d as u32));
        if self.integer { 0 } else { found.unwrap_or(DECIMALS) }
    }

    pub fn stride(&self) -> f64 {
        self.step.unwrap_or_else(|| 10f64.powi(-(self.places() as i32)))
    }

    pub fn fit(&self, v: f64) -> f64 {
        let v = v.clamp(self.min, self.max);
        let v =
            self.step.map_or(v, |s| (self.min + ((v - self.min) / s).round() * s).clamp(self.min, self.max));
        let scale = 10f64.powi(self.places() as i32);
        (v * scale).round() / scale
    }
}
