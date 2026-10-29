use crate::Color;
use crate::value::{Value, Vec2};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Tunable {
    Alpha,
    Size,
    Count,
    Speed,
    Life,
    TimeScale,
    Brightness,
    Color,
    Points,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Instance {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-range" = [0, 1]))]
    pub alpha: Option<Value<f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-range" = [0, 100]))]
    pub size: Option<Value<f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-range" = [0, 1]))]
    pub count: Option<Value<f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-range" = [0, 100]))]
    pub speed: Option<Value<f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-range" = [0, 100]))]
    pub life: Option<Value<f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-range" = [0, 100]))]
    pub time_scale: Option<Value<f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-range" = [0, 16]))]
    pub brightness: Option<Value<f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<Value<Color>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[schemars(extend("x-unit" = "px"))]
    pub points: Vec<Value<Vec2>>,
}

impl Instance {
    pub fn fields(&self) -> Vec<Tunable> {
        let numbers = [
            (Tunable::Alpha, &self.alpha),
            (Tunable::Size, &self.size),
            (Tunable::Count, &self.count),
            (Tunable::Speed, &self.speed),
            (Tunable::Life, &self.life),
            (Tunable::TimeScale, &self.time_scale),
            (Tunable::Brightness, &self.brightness),
        ];
        let mut set: Vec<Tunable> =
            numbers.into_iter().filter(|(_, v)| v.is_some()).map(|(t, _)| t).collect();
        set.extend(self.color.as_ref().map(|_| Tunable::Color));
        set.extend((!self.points.is_empty()).then_some(Tunable::Points));
        set
    }
}
