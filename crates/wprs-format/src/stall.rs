use crate::catalog::Catalog;
use crate::{Result, Text};
use serde::Serialize;
use std::path::Path;

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
