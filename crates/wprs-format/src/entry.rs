use crate::repo::hex64;
use crate::{
    DETAILED_FORMAT, Error, Result, Revoked, Root, SHARD_FORMAT, Shard, Time, ensure, require, sha256,
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
        let shards = self.shards.len() == entry.shards.len() && self.shards.iter().all(|b| hex64(&b.sha256));
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
    details.map_or((SHARD_FORMAT, &entry.shards), |d| (DETAILED_FORMAT, &d.shards))
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
        let shard: Shard = serde_json::from_slice(get(blob)?)?;
        shard.check(format, repo_id, index, entry.shard_bits).map(|()| shard)
    });
    let shards = shards.collect::<Result<Vec<_>>>()?;
    let revoked: Revoked = serde_json::from_slice(get(&entry.revoked)?)?;
    revoked.check(repo_id)?;
    Ok((shards, revoked))
}
