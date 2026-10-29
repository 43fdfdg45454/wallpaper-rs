use crate::meta::{Content, Credit, Links, Previews};
use crate::needs::{Input, Permissions, Platform, Requires};
use crate::origin::Origin;
use crate::video::Playback;
use crate::web::Web;
use crate::{Display, Error, Needs, Preset, Property, Result, Text, features};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use wprs_scene::Control;

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
    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        Self::read(bytes, true)
    }

    pub(crate) fn read(bytes: &[u8], strict: bool) -> Result<Self> {
        let header: Header = serde_json::from_slice(bytes)?;
        let major = header.schema.strip_prefix(PREFIX).and_then(|n| n.parse::<u32>().ok());
        if let Some(major) = major.filter(|m| *m > 2) {
            return Err(Error::Newer(format!("manifest version {major}")));
        }
        features::supported(header.requires.engine.as_deref(), &header.requires.features)?;
        let mut manifest: Manifest = serde_json::from_slice(bytes)?;
        if manifest.v2() {
            return Ok(manifest);
        }
        let newer = header.properties.iter().find(|(_, p)| !wprs_scene::legacy(p)).filter(|_| strict);
        if let Some((id, _)) = newer {
            let reason = "the newer kinds and fields of properties need wallpaper-rs.wpak/2".into();
            return Err(Error::Invalid { field: format!("properties.{id}"), reason });
        }
        for property in manifest.properties.values_mut() {
            if let Control::File { accept, .. } = &mut property.control {
                accept.retain(|a| a == "image");
            }
        }
        Ok(manifest)
    }

    pub(crate) fn newer_names(raw: &[u8], scene: Option<&str>, features: &BTreeSet<String>) -> Vec<String> {
        let parsed = |text: &str| serde_json::from_str(text).is_ok_and(|v| wprs_scene::newer_scene_keys(&v));
        let scene = || scene.is_some_and(parsed);
        serde_json::from_slice(raw).map_or(Vec::new(), |m| newer_names(&m, scene, features))
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
