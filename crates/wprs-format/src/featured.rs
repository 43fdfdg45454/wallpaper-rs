use crate::catalog::Catalog;
use crate::{Error, Result, ensure, require, valid_id};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

pub const FEATURED_FORMAT: &str = "wpak-repo-featured/1.0";
pub const MAX_FEATURED: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pick {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Featured {
    pub format: String,
    pub repo_id: String,
    pub items: Vec<Pick>,
}

impl Pick {
    fn valid(&self) -> bool {
        let note = |n: &str| (1..=200).contains(&n.chars().count()) && !n.contains(['<', '>', '\n', '\r']);
        valid_id(&self.id) && self.note.as_deref().is_none_or(note)
    }
}

impl Featured {
    pub fn parse(bytes: &[u8], repo_id: &str) -> Result<Featured> {
        let featured: Featured = serde_json::from_slice(bytes)?;
        let own = featured.format == FEATURED_FORMAT && featured.repo_id == repo_id;
        require(own, "featured", "expected a wpak-repo-featured/1.0 of this repository")?;
        let mut seen = HashSet::new();
        let items = &featured.items;
        let fine = items.len() <= MAX_FEATURED && items.iter().all(|p| p.valid() && seen.insert(&p.id));
        require(fine, "featured", "expected at most 64 package ids, each once, with short notes")?;
        Ok(featured)
    }
}

pub fn feature(dir: &Path, id: &str, note: Option<&str>, on: bool) -> Result<()> {
    let mut catalog = Catalog::load(dir)?;
    ensure(catalog.packages.iter().any(|l| l.id == id), || Error::NotFound(id.into()))?;
    let was = catalog.featured.len();
    catalog.featured.retain(|p| p.id != id);
    ensure(on || catalog.featured.len() < was, || Error::NotFound(format!("featured {id}")))?;
    if on {
        let pick =
            Pick { id: id.into(), note: note.map(str::trim).filter(|n| !n.is_empty()).map(String::from) };
        require(pick.valid(), "note", "has 1 to 200 characters on one line without < or >")?;
        require(catalog.featured.len() < MAX_FEATURED, "featured", "holds at most 64 packages")?;
        catalog.featured.push(pick);
    }
    catalog.save(dir)
}

pub fn featured(dir: &Path) -> Result<Vec<Pick>> {
    Ok(Catalog::load(dir)?.featured)
}
