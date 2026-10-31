use crate::meta::{Content, Credit, Links, Previews};
use crate::needs::{Input, Permissions, Platform, Requires};
use crate::origin::Origin;
use crate::video::Playback;
use crate::web::Web;
use crate::{Display, Needs, Preset, Property, Result, Text, features};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

mod kind;
mod read;

pub use kind::Kind;

pub const SCHEMA: &str = "wallpaper-rs.wpak/2";
pub const SCHEMA_V1: &str = "wallpaper-rs.wpak/1";
const PREFIX: &str = "wallpaper-rs.wpak/";

pub type Strings = BTreeMap<String, BTreeMap<String, String>>;

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
    #[serde(skip)]
    #[schemars(skip)]
    pub string_files: Strings,
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
    pub platforms: Vec<Platform>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<Input>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Permissions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web: Option<Web>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub playback: Option<Playback>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<Display>,
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
    #[serde(default)]
    properties: BTreeMap<String, serde_json::Value>,
}

pub fn newer_spelling(manifest: &serde_json::Value) -> Option<String> {
    let properties = manifest.get("properties")?.as_object()?;
    properties.iter().find(|(_, p)| !wprs_scene::legacy(p)).map(|(id, _)| id.clone())
}

pub fn newer_names(
    manifest: &serde_json::Value,
    scene: impl FnOnce() -> bool,
    features: &BTreeSet<String>,
) -> Vec<String> {
    let alone = manifest["schema"] == SCHEMA && features.iter().all(|f| features::released(f));
    let keys = alone && (wprs_scene::newer_manifest_keys(manifest) || scene());
    let names = [(newer_spelling(manifest).is_some(), "properties2"), (keys, "newer_keys")];
    names.into_iter().filter(|(on, _)| *on).map(|(_, name)| name.to_owned()).collect()
}

pub fn manifest_schema() -> serde_json::Value {
    serde_json::Value::from(schemars::schema_for!(Manifest))
}

impl Manifest {
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
