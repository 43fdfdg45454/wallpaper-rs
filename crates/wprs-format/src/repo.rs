use crate::{Error, KeyRevocation, Result, fingerprint, require, threshold};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub type Time = DateTime<Utc>;

pub const ROOT_FORMAT: &str = "wpak-repo-root/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Root {
    pub format: String,
    pub genesis: Option<String>,
    pub version: u64,
    pub expires: Time,
    pub name: String,
    pub keys: BTreeMap<String, Key>,
    pub roles: Roles,
    pub authors: Vec<Author>,
    pub policy: Policy,
    pub mirrors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Key {
    pub minisign: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Roles {
    pub root: Role,
    pub online: Role,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Role {
    pub keys: Vec<String>,
    pub threshold: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Author {
    pub id: String,
    pub keys: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub require_author_signature: bool,
    pub max_package_size: u64,
    pub entry_max_days: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authors: Option<String>,
}

impl Policy {
    pub fn online(&self) -> bool {
        self.authors.as_deref() == Some(crate::ONLINE)
    }
}

impl Root {
    pub fn check(&self, version: u64, repo_id: &str) -> Result<()> {
        require(self.format == ROOT_FORMAT, "root.format", "expected wpak-repo-root/1.0")?;
        require(self.version == version, "root.version", "does not match the file name")?;
        let genesis = self.genesis.as_deref() == (version > 1).then_some(repo_id);
        require(genesis, "root.genesis", "must be null in version 1 and the repository id after")?;
        let keys = self.keys.iter().all(|(fp, key)| fingerprint(&key.minisign).is_ok_and(|f| f == *fp));
        require(keys, "root.keys", "every key must be listed under its fingerprint")?;
        let roles = [&self.roles.root, &self.roles.online];
        let mut used = BTreeSet::new();
        let lists = roles.iter().map(|r| &r.keys).chain(self.authors.iter().map(|a| &a.keys));
        let unique = lists.flatten().all(|fp| self.keys.contains_key(fp) && used.insert(fp));
        require(unique, "root.roles", "keys must be listed and belong to one role or author")?;
        let counts = roles.iter().all(|r| r.keys.len() <= 8 && (1..=r.keys.len()).contains(&r.threshold));
        require(counts, "root.roles", "expected 1 to 8 keys and a threshold between 1 and their number")?;
        let mut ids = BTreeSet::new();
        let authors =
            self.authors.iter().all(|a| author_id(&a.id) && !a.keys.is_empty() && ids.insert(&a.id));
        require(authors, "root.authors", "expected unique lowercase ids with at least one key")?;
        let schemes = |m: &String| m.starts_with("https://") || m.starts_with("file://");
        require(
            self.mirrors.len() <= 16 && self.mirrors.iter().all(schemes),
            "root.mirrors",
            "expected at most 16 https:// or file:// URLs",
        )?;
        let policy = self.policy.authors.as_deref().is_none_or(|a| ["root", crate::ONLINE].contains(&a));
        require(policy, "root.policy", "authors must be root or online")?;
        require(
            (1..=90).contains(&self.policy.entry_max_days),
            "root.policy",
            "entry_max_days must be 1 to 90",
        )
    }

    pub fn material<'a>(&'a self, fingerprints: &'a [String]) -> Vec<(&'a str, &'a str)> {
        let known = fingerprints.iter().filter_map(|fp| self.keys.get(fp).map(|k| (fp, k)));
        known.map(|(fp, key)| (fp.as_str(), key.minisign.as_str())).collect()
    }

    pub fn meets(&self, role: &Role, data: &[u8], sigs: &str, comment: &str) -> Result<BTreeSet<String>> {
        threshold(data, sigs, comment, &self.material(&role.keys), role.threshold)
    }

    pub fn author_keys<'a>(
        &'a self,
        author: &'a Author,
        revoked: &[KeyRevocation],
    ) -> Vec<(&'a str, &'a str)> {
        let mut keys = self.material(&author.keys);
        keys.retain(|(fp, _)| revoked.iter().all(|r| r.fingerprint != *fp));
        keys
    }
}

pub fn comment(role: &str, version: u64, repo_id: &str) -> String {
    format!("wpak-repo {role} {version} {repo_id}")
}

pub(crate) fn at(now: i64) -> Result<Time> {
    DateTime::from_timestamp(now, 0)
        .ok_or_else(|| Error::Invalid { field: "time".into(), reason: "out of range".into() })
}

pub(crate) fn hex64(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub(crate) fn author_id(id: &str) -> bool {
    id.len() <= 32
        && id.starts_with(|c: char| c.is_ascii_lowercase())
        && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}
