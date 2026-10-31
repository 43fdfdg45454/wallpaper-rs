use crate::admin::{latest, load};
use crate::catalog::Catalog;
use crate::repo::at;
use crate::{Author, Key, Policy, ROOT_FORMAT, Result, Role, Roles, Root, fingerprint, public_key, require};
use chrono::TimeDelta;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    pub name: String,
    pub root_keys: Vec<String>,
    pub root_threshold: usize,
    pub online_keys: Vec<String>,
    pub authors: BTreeMap<String, Vec<String>>,
    pub require_author_signature: bool,
    pub root_days: u32,
    pub entry_max_days: u32,
    pub max_package_size: u64,
    pub mirrors: Vec<String>,
    pub shard_bits: u8,
    #[serde(default)]
    pub online_authors: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            name: String::new(),
            root_keys: Vec::new(),
            root_threshold: 1,
            online_keys: Vec::new(),
            authors: BTreeMap::new(),
            require_author_signature: false,
            root_days: 365,
            entry_max_days: 14,
            max_package_size: 1 << 30,
            mirrors: Vec::new(),
            shard_bits: 4,
            online_authors: false,
        }
    }
}

pub fn settings(dir: &Path) -> Result<Settings> {
    let root = load(dir, latest(dir)?.1)?;
    let keys = |fps: &[String]| root.material(fps).into_iter().map(|(_, k)| k.to_string()).collect();
    Ok(Settings {
        name: root.name.clone(),
        root_keys: keys(&root.roles.root.keys),
        root_threshold: root.roles.root.threshold,
        online_keys: keys(&root.roles.online.keys),
        authors: root.authors.iter().map(|a| (a.id.clone(), keys(&a.keys))).collect(),
        require_author_signature: root.policy.require_author_signature,
        entry_max_days: root.policy.entry_max_days,
        max_package_size: root.policy.max_package_size,
        mirrors: root.mirrors.clone(),
        shard_bits: Catalog::load(dir)?.shard_bits,
        online_authors: root.policy.online(),
        ..Settings::default()
    })
}

impl Settings {
    pub(crate) fn root(&self, version: u64, genesis: Option<String>, now: i64) -> Result<Root> {
        require((30..=1096).contains(&self.root_days), "root_days", "expected 30 to 1096 days")?;
        require(self.shard_bits <= 8, "shard_bits", "expected at most 8")?;
        let mut keys = BTreeMap::new();
        let mut add = |texts: &[String]| -> Result<Vec<String>> {
            let one = |text: &String| -> Result<String> {
                let minisign = public_key(text)?;
                let fp = fingerprint(&minisign)?;
                keys.insert(fp.clone(), Key { minisign });
                Ok(fp)
            };
            texts.iter().map(one).collect()
        };
        let root = Role { keys: add(&self.root_keys)?, threshold: self.root_threshold };
        let roles = Roles { root, online: Role { keys: add(&self.online_keys)?, threshold: 1 } };
        let authors = self.authors.iter().map(|(id, texts)| Ok(Author { id: id.clone(), keys: add(texts)? }));
        let authors = authors.collect::<Result<Vec<_>>>()?;
        let policy = Policy {
            require_author_signature: self.require_author_signature,
            max_package_size: self.max_package_size,
            entry_max_days: self.entry_max_days,
            authors: self.online_authors.then(|| crate::ONLINE.into()),
        };
        let (expires, repo_id) = (at(now)? + TimeDelta::days(self.root_days.into()), genesis.clone());
        let (format, name, mirrors) = (ROOT_FORMAT.into(), self.name.clone(), self.mirrors.clone());
        let root = Root { format, genesis, version, expires, name, keys, roles, authors, policy, mirrors };
        root.check(version, &repo_id.unwrap_or_default())?;
        Ok(root)
    }
}
