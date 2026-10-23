use crate::db::{ensure, printable, rows};
use crate::items::{Item, PackageInfo, item_in};
use crate::tags::{TAG_LENGTH, tag_item};
use crate::{Error, Kind, Library, Result};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use wprs_format::{Limits, Manifest, Text};

#[derive(Serialize, Deserialize)]
struct Titles {
    default: Option<String>,
    texts: BTreeMap<String, String>,
}

pub(crate) fn package(db: &Connection, id: i64, kind: Kind, path: &Path) -> Result<Option<Manifest>> {
    if kind != Kind::Package {
        return Ok(None);
    }
    let (manifest, facets, error) = match wprs_format::open(path, &Limits::default()) {
        Ok(package) => (Some(package.manifest), Some(package.facts.facets.join(" ")), None),
        Err(e) => (None, None, Some(e.to_string())),
    };
    let m = manifest.as_ref();
    let (pkg_id, version) = (m.map(|m| &m.id), m.map(|m| &m.version));
    let (pkg_type, license) = (m.map(|m| m.kind.to_string()), m.map(|m| &m.license));
    let content = m.and_then(|m| m.content.as_ref()).map(|c| c.rating.name());
    let (flashing, read) = (m.and_then(|m| m.photosensitive), wprs_core::VERSION);
    let titles = m.and_then(|m| match m.resolve(&m.title) {
        Text::Localized(texts) if texts.len() > 1 => {
            serde_json::to_string(&Titles { default: m.default_locale.clone(), texts }).ok()
        }
        _ => None,
    });
    let sql = "UPDATE items SET error = ?2, pkg_id = ?3, pkg_version = ?4, pkg_type = ?5, license = ?6, \
        content = ?7, photosensitive = ?8, facets = ?9, pkg_read = ?10, titles = ?11 WHERE id = ?1";
    let values =
        params![id, error, pkg_id, version, pkg_type, license, content, flashing, facets, read, titles];
    db.execute(sql, values)?;
    let usable = |tag: &&String| printable(tag.trim(), TAG_LENGTH) && !tag.contains(['<', '>']);
    for tag in m.iter().flat_map(|m| &m.tags).filter(usable) {
        tag_item(db, id, tag)?;
    }
    Ok(manifest)
}

impl Library {
    pub fn reread(&mut self) -> Result<u32> {
        let tx = self.db.transaction()?;
        let sql = "SELECT id, path, pkg_id IS NULL AND error IS NOT NULL FROM items \
            WHERE kind = 'package' AND missing = 0 AND pkg_read IS NOT ?1";
        let found: Vec<(i64, String, bool)> =
            rows(&tx, sql, [wprs_core::VERSION], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        for (id, path, unread) in &found {
            let path = Path::new(path);
            let Some(manifest) = package(&tx, *id, Kind::Package, path)?.filter(|_| *unread) else {
                continue;
            };
            let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            let title = manifest.title_in(&wprs_core::locale::languages());
            tx.execute("UPDATE items SET title = ?2 WHERE id = ?1 AND title = ?3", params![id, title, stem])?;
        }
        tx.commit()?;
        Ok(found.len() as u32)
    }

    pub fn localize(&self, items: &mut [Item], languages: &[String]) -> Result<()> {
        let sql = "SELECT uid, titles FROM items WHERE titles IS NOT NULL";
        let found: HashMap<String, String> =
            rows(&self.db, sql, [], |r| Ok((r.get(0)?, r.get(1)?)))?.into_iter().collect();
        for item in items.iter_mut() {
            let Some(titles) = found.get(&item.uid).and_then(|t| serde_json::from_str::<Titles>(t).ok())
            else {
                continue;
            };
            if titles.texts.values().any(|t| *t == item.title) {
                let chain = [languages, titles.default.as_slice()].concat();
                item.title = Text::Localized(titles.texts).pick(&chain);
            }
        }
        Ok(())
    }

    pub fn confirm(&mut self, uids: &[&str], what: &str) -> Result<Vec<String>> {
        let known = ["mature", "photosensitive"].contains(&what);
        ensure(known, || {
            Error::Invalid("confirmation", format!("{what:?} is not mature or photosensitive"))
        })?;
        let tx = self.db.transaction()?;
        let mut changed: Vec<String> = uids.iter().map(|uid| uid.to_string()).collect();
        for uid in uids {
            let sql = "INSERT OR IGNORE INTO confirmations(pkg_id, what) SELECT pkg_id, ?2 FROM items \
                WHERE uid = ?1 AND pkg_id IS NOT NULL";
            ensure(tx.execute(sql, params![uid, what])? == 1 || item_in(&tx, uid).is_ok(), || {
                Error::NotFound("item", uid.to_string())
            })?;
            let copies = "SELECT uid FROM items WHERE pkg_id = (SELECT pkg_id FROM items WHERE uid = ?1)";
            changed.extend(rows(&tx, copies, [uid], |row| row.get(0))?);
        }
        tx.commit()?;
        changed.sort();
        changed.dedup();
        Ok(changed)
    }

    pub fn unconfirmed_flashing(&self) -> Result<Vec<String>> {
        let sql = "SELECT uid FROM items WHERE photosensitive = 1 AND pkg_id NOT IN \
            (SELECT pkg_id FROM confirmations WHERE what = 'photosensitive')";
        rows(&self.db, sql, [], |row| row.get(0))
    }
}

impl PackageInfo {
    pub fn asks(&self, what: &str) -> bool {
        wprs_core::model::asks(what, self.content.as_deref(), self.photosensitive, &self.confirmed)
    }
}
