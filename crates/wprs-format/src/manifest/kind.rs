use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    #[default]
    Image,
    Video,
    Scene,
    Web,
    Preset,
    Assets,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Image => "image",
            Kind::Video => "video",
            Kind::Scene => "scene",
            Kind::Web => "web",
            Kind::Preset => "preset",
            Kind::Assets => "assets",
        }
    }

    pub fn shown(self) -> bool {
        !matches!(self, Kind::Preset | Kind::Assets)
    }

    pub fn first(self) -> bool {
        matches!(self, Kind::Image | Kind::Video | Kind::Scene)
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}
