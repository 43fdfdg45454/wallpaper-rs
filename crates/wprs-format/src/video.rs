use crate::display::FrameRate;
use crate::{Control, Manifest, Property, Text};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use wprs_scene::Slider;

pub const VIDEO_PROPERTIES: [&str; 7] = ["speed", "start", "end", "volume", "zoom", "x", "y"];
const MAX_START: f64 = 3600.0;
const TAIL: f64 = 0.5;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Playback {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_fps: Option<FrameRate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub muted: Option<bool>,
    #[serde(default, rename = "loop", skip_serializing_if = "Option::is_none")]
    pub repeat: Option<bool>,
}

fn within(value: Option<f64>, min: f64, max: f64) -> bool {
    value.is_none_or(|v| v.is_finite() && (min..=max).contains(&v))
}

impl Playback {
    pub(crate) fn valid(&self) -> bool {
        let ends = self.end.is_none_or(|end| end > self.start.unwrap_or(0.0));
        within(self.speed, 0.1, 4.0)
            && within(self.start, 0.0, MAX_START)
            && within(self.end, 0.0, MAX_START)
            && ends
            && within(self.volume, 0.0, 1.0)
            && self.max_fps.is_none_or(FrameRate::valid)
    }

    pub(crate) fn newer(&self) -> bool {
        self.max_fps.is_some() || self.muted.is_some() || self.repeat.is_some()
    }
}

impl Manifest {
    pub fn video_properties(&self, labels: &[String; 8], length: Option<f64>) -> BTreeMap<String, Property> {
        let (play, show) =
            (self.playback.clone().unwrap_or_default(), self.display.clone().unwrap_or_default());
        let length = length.filter(|l| l.is_finite() && *l >= 1.0).map_or(MAX_START, |l| l.min(MAX_START));
        let [x, y] = show.position.unwrap_or_default();
        let slider = |min: f64, max: f64, default: f64, places: u8, unit: &str| Slider {
            decimals: Some(places),
            editable: true,
            unit: Some(unit.to_owned()).filter(|u| !u.is_empty()),
            ..Slider::new(min, max, None, default.clamp(min, max))
        };
        let controls = [
            Slider { log: true, ..slider(0.1, 4.0, play.speed.unwrap_or(1.0), 2, "×") },
            slider(0.0, length - TAIL, play.start.unwrap_or(0.0), 1, "s"),
            slider(0.0, length, play.end.unwrap_or(0.0), 1, "s"),
            Slider {
                integer: true,
                step: Some(5.0),
                ..slider(0.0, 100.0, (play.volume.unwrap_or(1.0) * 20.0).round() * 5.0, 0, "%")
            },
            slider(1.0, 8.0, show.zoom.unwrap_or(1.0), 2, "×"),
            slider(-1.0, 1.0, x, 2, ""),
            slider(-1.0, 1.0, y, 2, ""),
        ];
        let named = VIDEO_PROPERTIES.iter().zip(controls).enumerate().map(|(i, (id, control))| {
            let help = (*id == "end").then(|| Text::Plain(labels[7].clone()));
            let label = Some(Text::Plain(labels[i].clone()));
            (
                id.to_string(),
                Property { label, help, order: i as f64, ..Property::new(Control::Slider(control)) },
            )
        });
        named.collect()
    }
}
