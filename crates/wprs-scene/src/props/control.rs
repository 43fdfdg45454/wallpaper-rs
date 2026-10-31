use super::slider::Slider;
use super::{Text, is_one, is_thousand, is_zero_depth, minute, thousand};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use std::ops::Not;

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
        #[serde(default, skip_serializing_if = "is_one")]
        mode: Mode,
        #[serde(default = "thousand", skip_serializing_if = "is_thousand")]
        #[schemars(extend("x-range" = [1, 1000]))]
        max_files: u32,
        #[serde(default, skip_serializing_if = "is_zero_depth")]
        #[schemars(extend("x-range" = [0, 2]))]
        recursive: u8,
    },
    Shortcut {},
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
pub enum Mode {
    #[default]
    One,
    All,
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
