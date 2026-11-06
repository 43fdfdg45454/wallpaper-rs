use crate::catalog::Catalog;
use crate::{Result, Text};
use serde::Serialize;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Stocked {
    pub id: String,
    pub author: Option<String>,
    pub revoked: bool,
    pub featured: bool,
    pub note: Option<String>,
    pub versions: Vec<Shelved>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Shelved {
    pub version: String,
    pub title: Text,
    pub listed: bool,
    pub revoked: bool,
}

pub fn stock(dir: &Path) -> Result<Vec<Stocked>> {
    let catalog = Catalog::load(dir)?;
    let revoked = |id: &str, version: &str| {
        let gone = |r: &crate::Revocation| r.id == id && r.versions.iter().any(|v| v == "*" || v == version);
        catalog.revoked.iter().any(gone)
    };
    let stocked = catalog.packages.iter().map(|listing| {
        let pick = catalog.featured.iter().find(|p| p.id == listing.id);
        let versions = listing.versions.iter().map(|r| Shelved {
            version: r.version.clone(),
            title: r.title.clone(),
            listed: r.shown(),
            revoked: revoked(&listing.id, &r.version),
        });
        Stocked {
            id: listing.id.clone(),
            author: listing.author.clone(),
            revoked: revoked(&listing.id, "*"),
            featured: pick.is_some(),
            note: pick.and_then(|p| p.note.clone()),
            versions: versions.collect(),
        }
    });
    Ok(stocked.collect())
}

pub fn opened(dir: &Path) -> Result<Value> {
    shown(dir)
}

pub fn added(dir: &Path, files: &[PathBuf], unlisted: bool, now: i64) -> Result<Value> {
    let mut done = Vec::new();
    for file in files {
        let added = crate::add(dir, file, None, now)?;
        if unlisted {
            crate::list(dir, &added.id, Some(&added.release.version), false)?;
        }
        done.push(format!("{} {}", added.id, added.release.version));
    }
    let mut state = opened(dir)?;
    state["added"] = json!(done);
    Ok(state)
}

#[cfg(feature = "sign")]
pub fn published(dir: &Path, signer: &crate::Signer, days: u32, now: i64) -> Result<Value> {
    let done = crate::publish(dir, signer, days, now)?;
    let mut state = opened(dir)?;
    state["published"] = serde_json::to_value(&done)?;
    Ok(state)
}

pub fn listed(dir: &Path, id: &str, version: Option<&str>, on: bool) -> Result<Value> {
    crate::list(dir, id, version, on)?;
    opened(dir)
}

pub fn featured(dir: &Path, id: &str, note: Option<&str>, on: bool) -> Result<Value> {
    crate::feature(dir, id, note, on)?;
    opened(dir)
}

fn fingerprints(keys: &[String]) -> Vec<String> {
    keys.iter().filter_map(|k| crate::fingerprint(k).ok()).collect()
}

fn shown(dir: &Path) -> Result<Value> {
    let (status, settings, catalog) = (crate::status(dir)?, crate::settings(dir)?, Catalog::load(dir)?);
    let authors: serde_json::Map<String, Value> =
        settings.authors.iter().map(|(id, keys)| (id.clone(), json!(fingerprints(keys)))).collect();
    Ok(json!({
        "name": settings.name, "repo_id": status.repo_id, "packages": stock(dir)?, "root": status,
        "root_keys": fingerprints(&settings.root_keys), "online_keys": fingerprints(&settings.online_keys),
        "authors": authors, "revoked_keys": catalog.keys.iter().map(|k| &k.fingerprint).collect::<Vec<_>>(),
        "policy": {
            "threshold": settings.root_threshold, "online_authors": settings.online_authors,
            "require_author_signature": settings.require_author_signature,
            "entry_max_days": settings.entry_max_days, "root_days": settings.root_days, "max_package_size": settings.max_package_size,
            "mirrors": settings.mirrors, "shard_bits": settings.shard_bits,
        },
    }))
}

fn reason(text: &str) -> Result<crate::Reason> {
    Ok(serde_json::from_value(json!(text))?)
}

pub fn revoked(dir: &Path, id: &str, version: Option<&str>, why: &str, message: &str) -> Result<Value> {
    crate::revoke(dir, id, version, reason(why)?, message)?;
    opened(dir)
}

pub fn unrevoked(dir: &Path, id: &str, version: &str) -> Result<Value> {
    crate::unrevoke(dir, id, version)?;
    opened(dir)
}

pub fn key_revoked(dir: &Path, fingerprint: &str, why: &str) -> Result<Value> {
    crate::revoke_key(dir, fingerprint, reason(why)?)?;
    opened(dir)
}

pub fn made(dir: &Path, charter: crate::Charter, now: i64) -> Result<Value> {
    let mut settings = crate::Settings::default();
    charter.apply(&mut settings)?;
    crate::init(dir, &settings, now)?;
    opened(dir)
}

pub fn rotated(dir: &Path, charter: crate::Charter, now: i64) -> Result<Value> {
    let mut settings = crate::settings(dir)?;
    charter.apply(&mut settings)?;
    crate::rotate_root(dir, &settings, now)?;
    opened(dir)
}

#[cfg(feature = "sign")]
pub fn root_signed(dir: &Path, signer: &crate::Signer) -> Result<Value> {
    crate::sign_root(dir, signer)?;
    opened(dir)
}
