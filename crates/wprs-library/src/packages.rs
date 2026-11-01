use crate::batch::{SPAN, batches};
use crate::db::{ensure, printable, rows};
use crate::items::{Item, PackageInfo, item_in};
use crate::tags::{TAG_LENGTH, offer_tag};
use crate::{Error, Kind, Library, Result};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use wprs_format::features::playable;
use wprs_format::{Limits, Manifest, Platform, Text};

#[derive(Serialize, Deserialize)]
struct Titles {
    default: Option<String>,
    texts: BTreeMap<String, String>,
}

pub(crate) struct Reading {
    pub(crate) manifest: Option<Manifest>,
    facets: Option<String>,
    error: Option<String>,
    base: Option<String>,
}

pub(crate) fn read_package(kind: Kind, path: &Path) -> Option<Reading> {
    (kind == Kind::Package).then_some(())?;
    let read = wprs_format::open(path, &Limits::default()).and_then(|mut p| p.look_closer().map(|()| p));
    Some(match read {
        Ok(mut package) => {
            let refused = playable(&package.manifest, &package.facts, Platform::here()).err();
            let (facets, error) = (Some(package.facts.facets.join(" ")), refused.map(|e| e.to_string()));
            let preset = package.manifest.kind == wprs_format::Kind::Preset;
            let base = preset.then(|| package.preset().ok()).flatten().map(|p| p.base.id);
            Reading { manifest: Some(package.manifest), facets, error, base }
        }
        Err(e) => Reading { manifest: None, facets: None, error: Some(e.to_string()), base: None },
    })
}

pub(crate) fn store_package(db: &Connection, id: i64, reading: &Reading) -> Result<()> {
    let m = reading.manifest.as_ref();
    let (pkg_id, version) = (m.map(|m| &m.id), m.map(|m| &m.version));
    let (pkg_type, license) = (m.map(|m| m.kind.to_string()), m.map(|m| &m.license));
    let content = m.and_then(|m| m.content.as_ref()).map(|c| c.rating.name());
    let (flashing, read, facets) = (m.and_then(|m| m.photosensitive), wprs_core::VERSION, &reading.facets);
    let titles = m.and_then(|m| match m.resolve(&m.title) {
        Text::Localized(texts) if texts.len() > 1 => {
            serde_json::to_string(&Titles { default: m.default_locale.clone(), texts }).ok()
        }
        _ => None,
    });
    let sql = "UPDATE items SET error = ?2, pkg_id = ?3, pkg_version = ?4, pkg_type = ?5, license = ?6, \
        content = ?7, photosensitive = ?8, facets = ?9, pkg_read = ?10, titles = ?11, base = ?12 WHERE id = ?1";
    let (error, base) = (&reading.error, &reading.base);
    let values =
        params![id, error, pkg_id, version, pkg_type, license, content, flashing, facets, read, titles, base];
    db.execute(sql, values)?;
    let usable = |tag: &&String| printable(tag.trim(), TAG_LENGTH) && !tag.contains(['<', '>']);
    for tag in m.iter().flat_map(|m| &m.tags).filter(usable) {
        offer_tag(db, id, tag)?;
    }
    Ok(())
}

impl Library {
    pub fn reread(&mut self) -> Result<u32> {
        let sql = "SELECT id, path, pkg_id IS NULL AND error IS NOT NULL FROM items \
            WHERE kind = 'package' AND missing = 0 AND pkg_read IS NOT ?1";
        let found: Vec<(i64, String, bool)> =
            rows(&self.db, sql, [wprs_core::VERSION], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        let mut read = |_: &Connection, (id, path, unread): &(i64, String, bool)| {
            Ok(read_package(Kind::Package, Path::new(path))
                .map(|reading| (*id, path.clone(), *unread, reading)))
        };
        let mut store = |tx: &Connection, (id, path, unread, reading): (i64, String, bool, Reading)| {
            store_package(tx, id, &reading)?;
            let Some(manifest) = reading.manifest.filter(|_| unread) else { return Ok(()) };
            let path = Path::new(&path);
            let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            let title = manifest.title_in(&wprs_core::locale::languages());
            let sql = "UPDATE items SET title = ?2 WHERE id = ?1 AND title = ?3";
            tx.execute(sql, params![id, title, stem]).map(drop).map_err(Into::into)
        };
        batches(&mut self.db, &found, SPAN, &mut read, &mut store, &mut |_| Ok(()))?;
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
        let sql = "SELECT count(*) FROM items WHERE uid = ?1 AND photosensitive = 1";
        for uid in uids.iter().filter(|_| what == "motion") {
            let flashes = self.db.query_row(sql, [uid], |row| row.get::<_, i64>(0))? > 0;
            ensure(!flashes, || {
                Error::Invalid("motion", "a package that flashes cannot keep moving".into())
            })?;
        }
        self.decide(
            uids,
            what,
            "INSERT OR IGNORE INTO confirmations(pkg_id, what) SELECT pkg_id, ?2 FROM items \
            WHERE uid = ?1 AND pkg_id IS NOT NULL",
        )
    }

    pub fn unconfirm(&mut self, uids: &[&str], what: &str) -> Result<Vec<String>> {
        self.decide(
            uids,
            what,
            "DELETE FROM confirmations WHERE what = ?2 AND pkg_id = \
            (SELECT pkg_id FROM items WHERE uid = ?1)",
        )
    }

    pub fn decided(&self, pkg_id: &str, what: &str) -> Result<bool> {
        let sql = "SELECT count(*) FROM confirmations WHERE pkg_id = ?1 AND what = ?2";
        Ok(self.db.query_row(sql, params![pkg_id, what], |row| row.get::<_, i64>(0))? > 0)
    }

    pub fn copies(&self, pkg_id: &str) -> Result<u32> {
        let sql = "SELECT count(*) FROM items WHERE pkg_id = ?1";
        Ok(self.db.query_row(sql, [pkg_id], |row| row.get::<_, u32>(0))?)
    }

    pub fn moves(&self, pkg_id: &str) -> Result<bool> {
        let sql = "SELECT count(*) FROM confirmations WHERE pkg_id = ?1 AND what = 'motion' \
            AND NOT EXISTS (SELECT 1 FROM items WHERE pkg_id = ?1 AND photosensitive = 1)";
        Ok(self.db.query_row(sql, [pkg_id], |row| row.get::<_, i64>(0))? > 0)
    }

    fn decide(&mut self, uids: &[&str], what: &str, sql: &str) -> Result<Vec<String>> {
        let known = DECISIONS.contains(&what);
        ensure(known, || {
            let known = DECISIONS.join(", ");
            Error::Invalid("confirmation", format!("{what:?} is not one of {known}"))
        })?;
        let tx = self.db.transaction()?;
        let mut changed: Vec<String> = uids.iter().map(|uid| uid.to_string()).collect();
        for uid in uids {
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

pub const DECISIONS: [&str; 7] =
    ["mature", "photosensitive", "motion", "hide_zone", "hide_language", "hide_gpu", "hide_storage"];

impl PackageInfo {
    pub fn asks(&self, what: &str) -> bool {
        wprs_core::model::asks(what, self.content.as_deref(), self.photosensitive, &self.confirmed)
    }
}
