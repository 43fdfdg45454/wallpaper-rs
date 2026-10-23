use crate::meta::{Content, Credit, Input, Links, Origin, Previews, Requires};
use crate::{Error, Needs, Preset, Property, Result, Text, features};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

pub const SCHEMA: &str = "wallpaper-rs.wpak/2";
pub const SCHEMA_V1: &str = "wallpaper-rs.wpak/1";
const PREFIX: &str = "wallpaper-rs.wpak/";

pub type Strings = BTreeMap<String, BTreeMap<String, String>>;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    #[default]
    Image,
    Video,
    Scene,
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Kind::Image => "image",
            Kind::Video => "video",
            Kind::Scene => "scene",
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: String,
    pub id: String,
    pub version: String,
    pub title: Text,
    #[serde(default, skip_serializing_if = "Text::is_empty")]
    #[schemars(extend("x-markup" = true))]
    pub description: Text,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-markup" = true))]
    pub changes: Option<Text>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-ref" = "language"))]
    pub default_locale: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub strings: Strings,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,
    pub license: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub credits: Vec<Credit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-required" = true))]
    pub origin: Option<Origin>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-required" = true))]
    pub content: Option<Content>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-required" = true))]
    pub photosensitive: Option<bool>,
    pub kind: Kind,
    pub entry: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-ref" = "thumbnail"))]
    pub preview: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previews: Option<Previews>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub links: Option<Links>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires: Option<Requires>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<Input>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, Property>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub presets: Vec<Preset>,
}

#[derive(Deserialize)]
struct Header {
    schema: String,
    #[serde(default)]
    requires: Needs,
}

pub fn manifest_schema() -> serde_json::Value {
    serde_json::Value::from(schemars::schema_for!(Manifest))
}

impl Manifest {
    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        let header: Header = serde_json::from_slice(bytes)?;
        let major = header.schema.strip_prefix(PREFIX).and_then(|n| n.parse::<u32>().ok());
        if let Some(major) = major.filter(|m| *m > 2) {
            return Err(Error::Newer(format!("manifest version {major}")));
        }
        features::supported(header.requires.engine.as_deref(), &header.requires.features)?;
        Ok(serde_json::from_slice(bytes)?)
    }

    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn v2(&self) -> bool {
        self.schema == SCHEMA
    }

    pub fn thumbnail(&self) -> Option<&String> {
        self.previews.as_ref().and_then(|p| p.thumbnail.as_ref()).or(self.preview.as_ref())
    }

    pub fn animated(&self) -> Option<&String> {
        self.previews.as_ref().and_then(|p| p.animated.as_ref())
    }

    pub fn features(&self) -> &[String] {
        self.requires.as_ref().map_or(&[], |r| &r.features)
    }
}
