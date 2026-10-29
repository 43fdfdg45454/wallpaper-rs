use crate::names::valid_id;
use schemars::JsonSchema;
use semver::VersionReq;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const MAX_PACKAGES: usize = 16;
pub const MAX_ORIGINS: usize = 8;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Requires {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(extend("x-computed" = true))]
    pub features: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub packages: Vec<Dependency>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu: Option<Gpu>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Dependency {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Gpu {
    Any,
    Compute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Input {
    Audio,
    Pointer,
    Clicks,
    Media,
    Network,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Desktop,
    Android,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Permissions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<Network>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub rgb: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Network {
    #[serde(default)]
    pub allow: Vec<String>,
}

impl Input {
    pub const ALL: [Input; 5] = [Input::Audio, Input::Pointer, Input::Clicks, Input::Media, Input::Network];

    pub fn name(self) -> &'static str {
        match self {
            Input::Audio => "audio",
            Input::Pointer => "pointer",
            Input::Clicks => "clicks",
            Input::Media => "media",
            Input::Network => "network",
        }
    }
}

impl Platform {
    pub const ALL: [Platform; 2] = [Platform::Desktop, Platform::Android];

    pub fn name(self) -> &'static str {
        match self {
            Platform::Desktop => "desktop",
            Platform::Android => "android",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Platform::Desktop => "the desktop",
            Platform::Android => "Android",
        }
    }

    pub fn here() -> Platform {
        if cfg!(target_os = "android") { Platform::Android } else { Platform::Desktop }
    }
}

impl Requires {
    pub(crate) fn valid_packages(&self, own: &str) -> bool {
        let mut ids = HashSet::new();
        let one = |d: &Dependency| {
            valid_id(&d.id)
                && d.id != own
                && ids.insert(d.id.clone())
                && VersionReq::parse(&d.version).is_ok()
        };
        self.packages.len() <= MAX_PACKAGES && self.packages.iter().all(one)
    }
}

impl Network {
    pub(crate) fn valid(&self) -> bool {
        let unique = self.allow.iter().collect::<HashSet<_>>().len() == self.allow.len();
        unique && self.allow.len() <= MAX_ORIGINS && self.allow.iter().all(|o| origin(o))
    }
}

pub fn origin(text: &str) -> bool {
    let Some(rest) = text.strip_prefix("https://") else { return false };
    let (host, port) = rest.split_once(':').unwrap_or((rest, "443"));
    let label = |l: &str| {
        (1..=63).contains(&l.len())
            && l.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    };
    host.len() <= 253
        && host.split('.').count() >= 2
        && host.split('.').all(label)
        && port.bytes().all(|b| b.is_ascii_digit())
        && port.parse::<u16>().is_ok_and(|p| p > 0)
}

impl crate::Manifest {
    pub fn declared(&self) -> Option<wprs_scene::Uses> {
        let has = |i: Input| self.inputs.contains(&i);
        let (audio, clicks, media) = (has(Input::Audio), has(Input::Clicks), has(Input::Media));
        let cursor = has(Input::Pointer) || clicks;
        self.v2().then_some(wprs_scene::Uses { audio, cursor, clicks, scripts: true, media })
    }
}
