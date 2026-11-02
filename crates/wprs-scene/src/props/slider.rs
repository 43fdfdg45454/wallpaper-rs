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
        let ends = |d: u32| [self.min, self.max].into_iter().filter_map(exact).fold(d, u32::max);
        let found = self.decimals.map(u32::from).or_else(|| self.step.and_then(exact).map(ends));
        if self.integer { 0 } else { found.unwrap_or(DECIMALS) }
    }

    pub fn stride(&self) -> f64 {
        self.step.unwrap_or_else(|| 10f64.powi(-(self.places() as i32)))
    }

    pub fn fit(&self, v: f64) -> f64 {
        let (low, high) = (self.min.min(self.max), self.max.max(self.min));
        let v = v.clamp(low, high);
        if v == low || v == high {
            return v;
        }
        let v = self.step.filter(|s| *s > 0.0).map_or(v, |s| low + ((v - low) / s).round() * s);
        let scale = 10f64.powi(self.places() as i32);
        ((v * scale).round() / scale).clamp(low, high)
    }
}

fn exact(x: f64) -> Option<u32> {
    (0..=6)
        .find(|d| {
            let scaled = x * 10f64.powi(*d);
            (scaled - scaled.round()).abs() < 1e-6 * scaled.abs().max(1.0)
        })
        .map(|d| d as u32)
}

#[cfg(test)]
mod tests {
    use super::Slider;

    #[test]
    fn the_ends_of_a_slider_are_always_accepted_and_nothing_leaves_them() {
        let parallax = Slider::new(0.0, 0.175, Some(0.01), 0.1);
        assert_eq!([parallax.fit(0.175), parallax.fit(0.0), parallax.fit(9.0)], [0.175, 0.0, 0.175]);
        assert_eq!([parallax.fit(0.174), parallax.fit(0.17), parallax.fit(0.1)], [0.17, 0.17, 0.1]);
        assert_eq!(parallax.places(), 3, "the shown value can say 0.175");
        let shifted = Slider::new(0.005, 0.995, Some(0.01), 0.5);
        assert_eq!([shifted.fit(0.015), shifted.fit(0.996), shifted.fit(0.984)], [0.015, 0.995, 0.985]);
        let mut few = Slider::new(0.0, 0.175, Some(0.01), 0.1);
        few.decimals = Some(1);
        assert_eq!([few.fit(0.175), few.fit(0.16), few.fit(0.12)], [0.175, 0.175, 0.1]);
        let free = Slider::new(-1.0, 1.0 / 3.0, None, 0.0);
        assert_eq!([free.fit(1.0 / 3.0), free.fit(0.3333)], [1.0 / 3.0, 0.33]);
        let grid = Slider::new(0.0, 1.0, Some(0.1), 0.5);
        assert_eq!(([grid.fit(0.34), grid.fit(0.96)], grid.places()), ([0.3, 1.0], 1));
        for v in (0..=400).map(|i| f64::from(i) / 1000.0 - 0.1) {
            let fitted = parallax.fit(v);
            assert!((0.0..=0.175).contains(&fitted), "{v} became {fitted}");
            assert_eq!(parallax.fit(fitted), fitted, "{v} fits once");
        }
    }
}
