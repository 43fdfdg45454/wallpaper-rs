use crate::names::{safe, valid_id, valid_license, within};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use wprs_core::markup::https;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Rating {
    General,
    Mature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Descriptor {
    Sexual,
    Violence,
    Gore,
    Language,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Content {
    pub rating: Rating,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub descriptors: Vec<Descriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Credit {
    pub paths: Vec<String>,
    pub license: String,
    pub attribution: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Links {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub support: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub donate: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Previews {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-ref" = "thumbnail"))]
    pub thumbnail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-ref" = "animated"))]
    pub animated: Option<String>,
}

impl Rating {
    pub fn name(self) -> &'static str {
        match self {
            Rating::General => "general",
            Rating::Mature => "mature",
        }
    }
}

impl Descriptor {
    pub fn name(self) -> &'static str {
        match self {
            Descriptor::Sexual => "sexual",
            Descriptor::Violence => "violence",
            Descriptor::Gore => "gore",
            Descriptor::Language => "language",
        }
    }
}

impl Content {
    pub(crate) fn valid(&self) -> bool {
        let unique = self.descriptors.iter().collect::<HashSet<_>>().len() == self.descriptors.len();
        unique && (self.rating == Rating::General || !self.descriptors.is_empty())
    }
}

impl Credit {
    pub(crate) fn valid(&self) -> bool {
        let source = |s: &Source| valid_id(&s.id) && semver::Version::parse(&s.version).is_ok();
        (1..=256).contains(&self.paths.len())
            && valid_license(&self.license)
            && within(&self.attribution, 256)
            && safe(&self.attribution)
            && self.paths.iter().all(|p| safe(p))
            && self.url.as_deref().is_none_or(https)
            && self.source.as_ref().is_none_or(source)
    }
}

impl Links {
    pub fn all(&self) -> [(&'static str, Option<&String>); 4] {
        let Links { homepage, source, support, donate } = self;
        [("homepage", homepage), ("source", source), ("support", support), ("donate", donate)]
            .map(|(name, url)| (name, url.as_ref()))
    }

    pub(crate) fn valid(&self) -> bool {
        self.all().iter().filter_map(|(_, url)| *url).all(|u| https(u))
    }
}
