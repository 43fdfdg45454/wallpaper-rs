use crate::{Result, Settings};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Charter {
    pub name: Option<String>,
    pub root_keys: Vec<String>,
    pub online_keys: Vec<String>,
    pub authors: BTreeMap<String, Vec<String>>,
    pub removed: Vec<String>,
    pub threshold: Option<usize>,
    pub require_author_signature: Option<bool>,
    pub online_authors: Option<bool>,
    pub root_days: Option<u32>,
    pub entry_max_days: Option<u32>,
    pub max_package_size: Option<u64>,
    pub mirrors: Option<Vec<String>>,
    pub shard_bits: Option<u8>,
}

pub fn key_text(key: &str) -> Result<String> {
    Ok(if Path::new(key).is_file() { std::fs::read_to_string(key)? } else { key.into() })
}

fn texts(keys: &[String]) -> Result<Vec<String>> {
    keys.iter().map(|k| key_text(k)).collect()
}

impl Charter {
    pub fn apply(self, s: &mut Settings) -> Result<()> {
        if let Some(name) = self.name {
            s.name = name;
        }
        if !self.root_keys.is_empty() {
            s.root_keys = texts(&self.root_keys)?;
        }
        if !self.online_keys.is_empty() {
            s.online_keys = texts(&self.online_keys)?;
        }
        for id in &self.removed {
            let found = s.authors.remove(id).is_some();
            crate::require(found, &format!("authors/{id}"), "is not an author of the root")?;
        }
        for (id, keys) in &self.authors {
            s.authors.insert(id.clone(), texts(keys)?);
        }
        s.root_threshold = self.threshold.unwrap_or(s.root_threshold);
        s.require_author_signature = self.require_author_signature.unwrap_or(s.require_author_signature);
        s.online_authors = self.online_authors.unwrap_or(s.online_authors);
        s.root_days = self.root_days.unwrap_or(s.root_days);
        s.entry_max_days = self.entry_max_days.unwrap_or(s.entry_max_days);
        s.max_package_size = self.max_package_size.unwrap_or(s.max_package_size);
        s.shard_bits = self.shard_bits.unwrap_or(s.shard_bits);
        if let Some(mirrors) = self.mirrors {
            s.mirrors = mirrors;
        }
        Ok(())
    }
}
