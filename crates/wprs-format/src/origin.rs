use crate::names::{safe, valid_license, within};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use wprs_core::markup::https;

pub const PROVIDER_LICENSES: [&str; 6] =
    ["CC0-1.0", "CC-PDM-1.0", "CC-PDDC", "CC-BY-2.0", "CC-BY-3.0", "CC-BY-4.0"];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum OriginKind {
    #[default]
    Original,
    Derived,
    Imported,
    Provider,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Origin {
    pub kind: OriginKind,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub derived_from: Vec<Ancestor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<Provider>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Ancestor {
    pub author: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Provider {
    pub name: String,
    pub item_url: String,
    pub credit: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terms_url: Option<String>,
}

impl Origin {
    pub fn by(authors: &[String]) -> Origin {
        let ancestor =
            |author: &String| Ancestor { author: author.clone(), title: None, url: None, license: None };
        let kind = if authors.is_empty() { OriginKind::Original } else { OriginKind::Derived };
        Origin { kind, derived_from: authors.iter().map(ancestor).collect(), ..Origin::default() }
    }

    pub(crate) fn newer(&self) -> bool {
        let kind = matches!(self.kind, OriginKind::Imported | OriginKind::Provider);
        kind || self.provider.is_some() || self.source_url.is_some()
    }

    pub(crate) fn valid(&self) -> bool {
        let count = match self.kind {
            OriginKind::Original => self.derived_from.is_empty(),
            OriginKind::Derived => !self.derived_from.is_empty(),
            OriginKind::Imported | OriginKind::Provider => true,
        };
        let valid = |a: &Ancestor| {
            within(&a.author, 128)
                && safe(&a.author)
                && a.title.as_deref().is_none_or(|t| within(t, 128) && safe(t))
                && a.url.as_deref().is_none_or(https)
                && a.license.as_deref().is_none_or(valid_license)
        };
        count && self.derived_from.len() <= 64 && self.derived_from.iter().all(valid)
    }

    pub(crate) fn sourced(&self) -> bool {
        let provider = (self.kind == OriginKind::Provider) == self.provider.is_some();
        provider
            && self.provider.as_ref().is_none_or(Provider::valid)
            && self.source_url.as_deref().is_none_or(https)
    }
}

impl Provider {
    fn valid(&self) -> bool {
        let token = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-';
        (1..=32).contains(&self.name.len())
            && self.name.bytes().all(token)
            && https(&self.item_url)
            && within(&self.credit, 300)
            && safe(&self.credit)
            && self.terms_url.as_deref().is_none_or(https)
    }
}

impl crate::Manifest {
    pub fn publishable(&self) -> crate::Result<()> {
        let kind = self.origin.as_ref().map(|o| o.kind);
        let reason = "imported packages are for personal use and are never published";
        crate::require(kind != Some(OriginKind::Imported), "origin.kind", reason)?;
        let open = |branch: &str| {
            branch.split(" AND ").all(|id| PROVIDER_LICENSES.contains(&id.trim_matches(['(', ')', ' '])))
        };
        let free = kind != Some(OriginKind::Provider) || self.license.split(" OR ").any(open);
        let reason = format!("a package from a provider is published under {}", PROVIDER_LICENSES.join(", "));
        crate::require(free, "license", &reason)
    }
}
