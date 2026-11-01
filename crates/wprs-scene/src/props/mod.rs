mod check;
mod cond;
mod expr;
mod legacy;
mod leveling;
mod slider;
mod text;
mod value;
mod values;

pub use check::{ICONS, check_presets, cycle, icon};
pub use expr::{Expr, rename};
pub use legacy::legacy;
pub use leveling::Leveling;
pub use slider::Slider;
pub use text::tag;
pub use values::{effective, exposed, stale};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use std::collections::BTreeMap;

mod control;

pub use control::{Choice, Control, Mode, Pick};

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
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub quality: BTreeMap<crate::Quality, Json>,
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

fn thousand() -> u32 {
    1000
}

fn is_thousand(v: &u32) -> bool {
    *v == 1000
}

fn is_one(m: &Mode) -> bool {
    *m == Mode::One
}

fn is_zero_depth(v: &u8) -> bool {
    *v == 0
}
