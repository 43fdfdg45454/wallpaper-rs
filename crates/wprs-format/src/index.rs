use crate::manifest::valid_id;
use crate::repo::hex64;
use crate::{Kind, Result, Time, require};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const SHARD_FORMAT: &str = "wpak-repo-shard/1.0";
pub const REVOKED_FORMAT: &str = "wpak-repo-revoked/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Shard {
    pub format: String,
    pub repo_id: String,
    pub shard: usize,
    pub packages: Vec<Listing>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Listing {
    pub id: String,
    pub author: Option<String>,
    pub versions: Vec<Release>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Release {
    pub version: String,
    pub kind: Kind,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub license: String,
    pub authors: Vec<String>,
    pub size: u64,
    pub sha256: String,
    pub manifest_sha256: String,
    pub preview: Option<Preview>,
    pub published: Time,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preview {
    pub sha256: String,
    pub size: u64,
    pub ext: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Revoked {
    pub format: String,
    pub repo_id: String,
    pub packages: Vec<Revocation>,
    pub keys: Vec<KeyRevocation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Revocation {
    pub id: String,
    pub versions: Vec<String>,
    pub reason: Reason,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyRevocation {
    pub fingerprint: String,
    pub reason: Reason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Reason {
    Malware,
    License,
    Takedown,
    Broken,
    Other,
}

impl Shard {
    pub fn check(&self, repo_id: &str, index: usize, bits: u8) -> Result<()> {
        let own = self.format == SHARD_FORMAT && self.repo_id == repo_id && self.shard == index;
        require(own, "shard", "does not match its place in the entry")?;
        let mut ids = BTreeSet::new();
        self.packages.iter().try_for_each(|p| {
            let placed = valid_id(&p.id) && shard_of(&p.id, bits) == index && ids.insert(&p.id);
            require(placed, "shard.id", &format!("{:?} is invalid, misplaced or repeated", p.id))?;
            let versions: Option<Vec<Version>> = p.versions.iter().map(Release::parse).collect();
            let sorted = versions.is_some_and(|v| !v.is_empty() && v.is_sorted_by(|a, b| a < b));
            require(sorted, "shard.versions", &format!("{:?} needs unique ascending valid releases", p.id))
        })
    }
}

impl Release {
    fn parse(&self) -> Option<Version> {
        let preview = self.preview.as_ref();
        let hashes = [&self.sha256, &self.manifest_sha256].into_iter().chain(preview.map(|p| &p.sha256));
        let ext = preview.is_none_or(|p| {
            (1..=8).contains(&p.ext.len())
                && p.ext.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        });
        let version = Version::parse(&self.version).ok().filter(|v| v.to_string() == self.version);
        version.filter(|_| ext && hashes.into_iter().all(|h| hex64(h)))
    }
}

impl Revoked {
    pub fn check(&self, repo_id: &str) -> Result<()> {
        let own = self.format == REVOKED_FORMAT && self.repo_id == repo_id;
        require(own, "revoked", "expected a wpak-repo-revoked/1.0 of this repository")
    }

    pub fn version(&self, id: &str, version: &str) -> Option<Reason> {
        let hit = |r: &&Revocation| r.id == id && r.versions.iter().any(|v| v == "*" || v == version);
        self.packages.iter().find(hit).map(|r| r.reason)
    }

    pub fn key(&self, fingerprint: &str) -> Option<Reason> {
        self.keys.iter().find(|k| k.fingerprint == fingerprint).map(|k| k.reason)
    }
}

pub fn shard_of(id: &str, bits: u8) -> usize {
    usize::from(Sha256::digest(id.as_bytes())[0]) >> (8 - bits.min(8))
}
