use crate::{Error, Preset, Property, Result, require};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

pub const SCHEMA: &str = "wallpaper-rs.wpak/1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: String,
    pub id: String,
    pub version: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,
    pub license: String,
    pub kind: Kind,
    pub entry: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, Property>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub presets: Vec<Preset>,
}

impl Manifest {
    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        Ok(serde_json::from_slice(bytes)?)
    }

    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn check(&self) -> Result<()> {
        require(self.schema == SCHEMA, "schema", "expected wallpaper-rs.wpak/1")?;
        require(valid_id(&self.id), "id", "expected a lowercase reverse-DNS name like org.example.name")?;
        require(semver::Version::parse(&self.version).is_ok(), "version", "expected a semantic version")?;
        require(within(&self.title, 128), "title", "expected 1 to 128 characters")?;
        require(self.authors.iter().all(|a| within(a, 128)), "authors", "expected 1 to 128 characters each")?;
        require(valid_license(&self.license), "license", "expected an SPDX license expression")?;
        let tags = self.tags.len() <= 32 && self.tags.iter().all(|t| within(t, 32));
        require(tags, "tags", "expected at most 32 tags of 1 to 32 characters")?;
        require(
            self.properties.len() <= 64 && self.presets.len() <= 32,
            "properties",
            "too many properties or presets",
        )?;
        self.properties.iter().try_for_each(|(name, property)| {
            let field = format!("properties.{name}");
            require(valid_name(name), &field, "expected a lowercase identifier")?;
            property.check().map_err(|reason| Error::Invalid { field, reason })
        })?;
        wprs_scene::check_presets(&self.properties, &self.presets)
            .map_err(|reason| Error::Invalid { field: "presets".into(), reason })
    }
}

fn within(text: &str, max: usize) -> bool {
    !text.trim().is_empty() && text.chars().count() <= max
}

fn valid_name(name: &str) -> bool {
    name.len() <= 63
        && name.starts_with(|c: char| c.is_ascii_lowercase())
        && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

pub(crate) fn valid_id(id: &str) -> bool {
    id.len() <= 255 && id.split('.').count() >= 2 && id.split('.').all(valid_name)
}

#[derive(Clone, Copy, PartialEq)]
enum Next {
    Operand,
    Exception,
    Operator { with: bool },
}

fn valid_license(expr: &str) -> bool {
    let spaced = expr.replace('(', " ( ").replace(')', " ) ");
    let end = spaced.split_whitespace().try_fold((Next::Operand, 0usize), |(next, depth), token| {
        match (token, next) {
            ("(", Next::Operand) => Some((Next::Operand, depth + 1)),
            (")", Next::Operator { .. }) if depth > 0 => Some((Next::Operator { with: false }, depth - 1)),
            ("AND" | "OR", Next::Operator { .. }) => Some((Next::Operand, depth)),
            ("WITH", Next::Operator { with: true }) => Some((Next::Exception, depth)),
            ("(" | ")" | "AND" | "OR" | "WITH", _) | (_, Next::Operator { .. }) => None,
            (id, _) => {
                let id = id.strip_suffix('+').unwrap_or(id);
                let valid = id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | ':'));
                (valid && !id.is_empty()).then_some((Next::Operator { with: next == Next::Operand }, depth))
            }
        }
    });
    matches!(end, Some((Next::Operator { .. }, 0)))
}
