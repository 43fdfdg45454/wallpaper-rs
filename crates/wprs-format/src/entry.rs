use crate::repo::hex64;
use crate::{
    DETAILED_FORMAT, Error, KINDS_FORMAT, Result, Revoked, Root, SHARD_FORMAT, Shard, Time, ensure, require,
    sha256,
};
use chrono::TimeDelta;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const ENTRY_FORMAT: &str = "wpak-repo-entry/1.0";
pub const DETAILS_FORMAT: &str = "wpak-repo-details/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub format: String,
    pub repo_id: String,
    pub version: u64,
    pub timestamp: Time,
    pub expires: Time,
    pub root: Pin,
    pub shard_bits: u8,
    pub shards: Vec<Blob>,
    pub revoked: Blob,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Details {
    pub format: String,
    pub repo_id: String,
    pub version: u64,
    pub entry: String,
    pub shards: Vec<Blob>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub kinds: Vec<Blob>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pin {
    pub version: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Blob {
    pub sha256: String,
    pub size: u64,
}

impl Entry {
    pub fn check(&self, version: u64, repo_id: &str, root: &Root, root_sha256: &str) -> Result<()> {
        let own = self.format == ENTRY_FORMAT && self.repo_id == repo_id && self.version == version;
        require(own, "entry.version", "does not match its file name or repository")?;
        let pin = Pin { version: root.version, sha256: root_sha256.into() };
        require(self.root == pin, "entry.root", "does not pin the current root")?;
        let blobs = self.shards.iter().chain([&self.revoked]).all(|b| hex64(&b.sha256));
        let shards = self.shard_bits <= 8 && self.shards.len() == 1 << self.shard_bits;
        require(blobs && shards, "entry.shards", "expected 2^shard_bits shards named by SHA-256")?;
        let life = self.expires - self.timestamp;
        let max = TimeDelta::days(root.policy.entry_max_days.into());
        require(
            life > TimeDelta::zero() && life <= max,
            "entry.expires",
            "exceeds the lifetime the root allows",
        )
    }
}

impl Details {
    pub fn check(&self, entry: &Entry, entry_sha256: &str) -> Result<()> {
        let own = self.format == DETAILS_FORMAT && self.repo_id == entry.repo_id;
        let pinned = self.version == entry.version && self.entry == entry_sha256;
        let kinds = self.kinds.is_empty() || self.kinds.len() == entry.shards.len();
        let blobs = self.shards.iter().chain(&self.kinds).all(|b| hex64(&b.sha256));
        let shards = self.shards.len() == entry.shards.len() && kinds && blobs;
        require(own && pinned && shards, "details", "does not match the entry it extends")
    }
}

impl Blob {
    pub fn matches(&self, bytes: &[u8]) -> Result<()> {
        let ok = bytes.len() as u64 == self.size && sha256(bytes)? == self.sha256;
        ensure(ok, || Error::Mismatch(self.sha256.clone()))
    }
}

pub(crate) fn shards<'a>(entry: &'a Entry, details: Option<&'a Details>) -> (&'static str, &'a [Blob]) {
    match details {
        Some(d) if !d.kinds.is_empty() => (KINDS_FORMAT, &d.kinds),
        Some(d) => (DETAILED_FORMAT, &d.shards),
        None => (SHARD_FORMAT, &entry.shards),
    }
}

pub(crate) fn kept<'a>(entry: &'a Entry, details: Option<&'a Details>) -> impl Iterator<Item = &'a Blob> {
    let older = details.filter(|d| !d.kinds.is_empty()).map_or(&[][..], |d| &d.shards);
    shards(entry, details).1.iter().chain(older)
}

pub(crate) fn cold(details: Option<&Details>, blobs: &BTreeMap<String, Vec<u8>>) -> Option<Details> {
    let details = details.filter(|d| d.kinds.iter().any(|b| !blobs.contains_key(&b.sha256)))?;
    Some(Details { kinds: Vec::new(), ..details.clone() })
}

pub(crate) fn contents(
    entry: &Entry,
    details: Option<&Details>,
    repo_id: &str,
    blobs: &BTreeMap<String, Vec<u8>>,
) -> Result<(Vec<Shard>, Revoked)> {
    let get = |blob: &Blob| blobs.get(&blob.sha256).ok_or_else(|| Error::Missing(blob.sha256.clone()));
    let (format, list) = shards(entry, details);
    let shards = list.iter().enumerate().map(|(index, blob)| {
        let shard = Shard::parse(get(blob)?)?;
        shard.check(format, repo_id, index, entry.shard_bits).map(|()| shard)
    });
    let shards = shards.collect::<Result<Vec<_>>>()?;
    let revoked: Revoked = serde_json::from_slice(get(&entry.revoked)?)?;
    revoked.check(repo_id)?;
    Ok((shards, revoked))
}
