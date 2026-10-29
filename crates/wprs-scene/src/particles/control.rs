use super::Follow;
use crate::value::{Value, Vec2};
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

pub const MAX_CONTROL: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Handle(pub u8);

impl JsonSchema for Handle {
    fn schema_name() -> Cow<'static, str> {
        "Handle".into()
    }
    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        schemars::json_schema!({"type": "string", "pattern": "^p[0-7]$"})
    }
}

impl TryFrom<String> for Handle {
    type Error = String;
    fn try_from(text: String) -> Result<Self, String> {
        let n = text.strip_prefix('p').and_then(|n| n.parse::<u8>().ok()).filter(|n| *n < MAX_CONTROL as u8);
        n.filter(|n| text == format!("p{n}")).map(Handle).ok_or(format!("expected p0 to p7, found {text:?}"))
    }
}

impl From<Handle> for String {
    fn from(h: Handle) -> String {
        format!("p{}", h.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Placement {
    #[default]
    Local,
    World,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ControlPoint {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-unit" = "px"))]
    pub at: Option<Value<Vec2>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub follow: Option<Follow>,
    #[serde(default)]
    pub space: Placement,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-unit" = "°"))]
    pub angle: Option<Value<f32>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Periodic {
    #[schemars(extend("x-unit" = "s"))]
    pub on: [f32; 2],
    #[schemars(extend("x-unit" = "s"))]
    pub off: [f32; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Perspective {
    #[schemars(extend("x-unit" = "px", "x-range" = [1, 100000]))]
    pub focal: f32,
}
