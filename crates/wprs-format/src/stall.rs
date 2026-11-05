use crate::catalog::Catalog;
use crate::{Result, Text};
use serde::Serialize;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Stocked {
    pub id: String,
    pub author: Option<String>,
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
            featured: pick.is_some(),
            note: pick.and_then(|p| p.note.clone()),
            versions: versions.collect(),
        }
    });
    Ok(stocked.collect())
}

pub fn opened(dir: &Path) -> Result<Value> {
    let (status, name) = (crate::status(dir)?, crate::settings(dir)?.name);
    Ok(json!({ "name": name, "repo_id": status.repo_id, "packages": stock(dir)? }))
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
