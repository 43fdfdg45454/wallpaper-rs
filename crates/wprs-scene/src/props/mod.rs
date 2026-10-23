mod check;
mod cond;
mod expr;
mod legacy;
mod slider;
mod text;
mod value;
mod values;

pub use check::{ICONS, check_presets, cycle, icon};
pub use expr::{Expr, rename};
pub use legacy::legacy;
pub use slider::Slider;
pub use text::tag;
pub use values::{effective, exposed, stale};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use std::collections::BTreeMap;
use std::ops::Not;

pub type Values = BTreeMap<String, Json>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Text {
    Plain(String),
    Localized(BTreeMap<String, String>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Property {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<Text>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub help: Option<Text>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-ref" = "group"))]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Condition::is_empty")]
    #[schemars(extend("x-condition" = true))]
    pub visible_if: Condition,
    #[serde(default, skip_serializing_if = "Condition::is_empty")]
    #[schemars(extend("x-condition" = true))]
    pub enabled_if: Condition,
    #[serde(default, skip_serializing_if = "is_zero", serialize_with = "whole")]
    pub order: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-ref" = "icon"))]
    pub icon: Option<String>,
    #[serde(flatten)]
    pub control: Control,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Condition {
    Expr(String),
    Equal(Values),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
pub enum Control {
    Color {
        #[schemars(extend("x-color" = true))]
        default: String,
        #[serde(default)]
        alpha: bool,
    },
    Slider(Slider),
    Bool {
        default: bool,
    },
    Combo {
        options: Vec<Choice>,
        #[schemars(extend("x-ref" = "option"))]
        default: Json,
    },
    Text {
        #[serde(default)]
        default: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-range" = [1, 1024]))]
        max_len: Option<u32>,
        #[serde(default)]
        multiline: bool,
        #[serde(default, skip_serializing_if = "Not::not")]
        secret: bool,
    },
    File {
        #[serde(default)]
        #[schemars(extend("items" = {"type": "string", "enum": ["image", "video"]}))]
        accept: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-ref" = "media"))]
        default_asset: Option<String>,
    },
    Folder {
        #[serde(default)]
        #[schemars(extend("items" = {"type": "string", "enum": ["image", "video"]}))]
        accept: Vec<String>,
        #[serde(default)]
        pick: Pick,
        #[serde(default = "minute")]
        #[schemars(extend("x-unit" = "s", "x-range" = [10, 86400]))]
        every: u32,
    },
    Button {},
    Group {
        #[serde(default, skip_serializing_if = "Not::not")]
        collapsed: bool,
    },
    Label {},
    Divider {},
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Pick {
    #[default]
    Random,
    Sequence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Choice {
    Labeled {
        #[schemars(extend("x-scalar" = true))]
        value: Json,
        label: Text,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(extend("x-ref" = "icon"))]
        icon: Option<String>,
    },
    Plain(Json),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Preset {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<Text>,
    pub values: Values,
}

impl Default for Condition {
    fn default() -> Self {
        Condition::Equal(Values::new())
    }
}

fn whole<S: serde::Serializer>(v: &f64, s: S) -> Result<S::Ok, S::Error> {
    if v.fract() == 0.0 && v.abs() < 1e15 { s.serialize_i64(*v as i64) } else { s.serialize_f64(*v) }
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

fn minute() -> u32 {
    60
}
