use crate::value::Param;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MAX_SCRIPTS2: usize = 64;
pub const MAX_SCRIPT_PARAMS: usize = 32;
pub const MAX_TEMPLATES: usize = 64;
pub const MAX_POOLS: usize = 16;
pub const MAX_POOL_LAYERS: u32 = 256;
pub const MAX_POOL_VERTICES: u32 = 65_536;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Script {
    pub path: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<ScriptEvent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub uses: Vec<ScriptApi>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, ScriptParam>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ScriptEvent {
    Init,
    Update,
    Destroy,
    Resize,
    Properties,
    Settings,
    TimelineEvent,
    Click,
    Enter,
    Leave,
    Move,
    Down,
    Up,
    MediaStatus,
    MediaPlayback,
    MediaProperties,
    MediaThumbnail,
    MediaTimeline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ScriptApi {
    Audio,
    Pointer,
    Clicks,
    Media,
    Timers,
    Storage,
    Layers,
    Geometry,
    Effects,
    Particles,
    Sounds,
    Animations,
    Camera,
    Math,
    Shortcuts,
    Console,
    Shared,
    Hierarchy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ParamType {
    Number,
    Bool,
    Text,
    Color,
    Vec2,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScriptParam {
    #[serde(rename = "type")]
    pub kind: ParamType,
    pub default: Param,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PoolKind {
    Mesh,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Pool {
    #[serde(rename = "type")]
    pub kind: PoolKind,
    #[schemars(extend("x-range" = [1, 256]))]
    pub max: u32,
    #[schemars(extend("x-range" = [3, 65536]))]
    pub max_vertices: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-ref" = "image"))]
    pub texture: Option<String>,
}

impl ScriptEvent {
    pub fn newer(self) -> bool {
        self != ScriptEvent::Update
    }

    pub fn pointer(self) -> bool {
        matches!(self, ScriptEvent::Enter | ScriptEvent::Leave | ScriptEvent::Move)
    }

    pub fn clicks(self) -> bool {
        matches!(self, ScriptEvent::Click | ScriptEvent::Down | ScriptEvent::Up)
    }

    pub fn media(self) -> bool {
        matches!(
            self,
            ScriptEvent::MediaStatus
                | ScriptEvent::MediaPlayback
                | ScriptEvent::MediaProperties
                | ScriptEvent::MediaThumbnail
                | ScriptEvent::MediaTimeline
        )
    }
}

impl ScriptParam {
    pub fn accepts(&self, v: &Param) -> bool {
        matches!(
            (self.kind, v),
            (ParamType::Number, Param::Number(_))
                | (ParamType::Bool, Param::Bool(_))
                | (ParamType::Text, Param::Text(_))
                | (ParamType::Color, Param::Color(_))
                | (ParamType::Vec2, Param::Vec2(_))
        )
    }
}

impl Script {
    pub fn new(path: impl Into<String>) -> Script {
        Script { path: path.into(), events: Vec::new(), uses: Vec::new(), params: BTreeMap::new() }
    }

    pub fn wants(&self, api: ScriptApi) -> bool {
        self.uses.contains(&api)
    }
}
