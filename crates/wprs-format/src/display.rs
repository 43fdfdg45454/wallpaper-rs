use crate::web::rgb;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Display {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>", extend("enum" = ["fill", "fit", "stretch", "center", "tile", "span"]))]
    pub fit: Option<wprs_core::Fit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zoom: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-color" = true))]
    pub background: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flip: Option<Flip>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent: Option<Accent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Span {
    PerOutput,
    AllOutputs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Flip {
    None,
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Accent {
    #[schemars(extend("x-color" = true))]
    Color(String),
    Prop {
        #[schemars(extend("x-ref" = "property"))]
        prop: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum FrameRate {
    #[schemars(extend("x-range" = [1, 240]))]
    Fixed(u32),
    Follow(Follow),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Follow {
    Display,
}

fn within(value: Option<f64>, min: f64, max: f64) -> bool {
    value.is_none_or(|v| v.is_finite() && (min..=max).contains(&v))
}

impl Display {
    pub(crate) fn valid(&self) -> bool {
        let [x, y] = self.position.map_or([None; 2], |p| p.map(Some));
        let accent = match &self.accent {
            Some(Accent::Color(c)) => rgb(c),
            _ => true,
        };
        within(self.zoom, 1.0, 8.0)
            && within(x, -1.0, 1.0)
            && within(y, -1.0, 1.0)
            && self.background.as_deref().is_none_or(rgb)
            && accent
    }

    pub(crate) fn newer(&self) -> bool {
        self.background.is_some() || self.span.is_some() || self.flip == Some(Flip::None)
    }

    pub(crate) fn flipped(&self) -> bool {
        self.flip.is_some_and(|f| f != Flip::None)
    }
}

impl FrameRate {
    pub(crate) fn valid(self) -> bool {
        match self {
            FrameRate::Fixed(fps) => (1..=240).contains(&fps),
            FrameRate::Follow(_) => true,
        }
    }
}
