use crate::repo::hex64;
use crate::{Content, Dependency, Kind, Requires, Text, Time};
use semver::Version;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Release {
    pub version: String,
    pub kind: Kind,
    pub title: Text,
    pub description: Text,
    pub tags: Vec<String>,
    pub license: String,
    pub authors: Vec<String>,
    pub size: u64,
    pub sha256: String,
    pub manifest_sha256: String,
    pub preview: Option<Preview>,
    pub published: Time,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub schema: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_locale: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changes: Option<Text>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<Rated>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub photosensitive: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires: Option<Needs>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub facets: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animated: Option<Preview>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub platforms: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preview {
    pub sha256: String,
    pub size: u64,
    pub ext: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rated {
    pub rating: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub descriptors: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Needs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub packages: Vec<Wanted>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wanted {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub version: String,
}

impl Rated {
    pub fn mature(&self) -> bool {
        self.rating != "general"
    }
}

impl From<&Content> for Rated {
    fn from(content: &Content) -> Rated {
        let descriptors = content.descriptors.iter().map(|d| d.name().into()).collect();
        Rated { rating: content.rating.name().into(), descriptors }
    }
}

impl From<&Requires> for Needs {
    fn from(requires: &Requires) -> Needs {
        let wanted = |d: &Dependency| Wanted { id: d.id.clone(), version: d.version.clone() };
        let packages = requires.packages.iter().map(wanted).collect();
        Needs { engine: requires.engine.clone(), features: requires.features.clone(), packages }
    }
}

impl Release {
    pub(crate) fn parse(&self) -> Option<Version> {
        let previews = self.preview.iter().chain(&self.animated);
        let hashes =
            [&self.sha256, &self.manifest_sha256].into_iter().chain(previews.clone().map(|p| &p.sha256));
        let ext = previews.clone().all(|p| {
            (1..=8).contains(&p.ext.len())
                && p.ext.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        });
        let version = Version::parse(&self.version).ok().filter(|v| v.to_string() == self.version);
        version.filter(|_| ext && hashes.into_iter().all(|h| hex64(h)))
    }

    pub fn first(&self) -> bool {
        self.schema.is_empty()
    }

    pub fn title_in(&self, languages: &[String]) -> String {
        self.title.pick(&[languages, self.default_locale.as_slice()].concat())
    }

    pub fn description_in(&self, languages: &[String]) -> String {
        self.description.pick(&[languages, self.default_locale.as_slice()].concat())
    }
}
