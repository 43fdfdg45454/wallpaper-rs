use crate::db::{ensure, rows};
use crate::{Error, Kind, Library, Result};
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub(crate) const SELECT_ITEMS: &str = "SELECT i.*, s.uid AS source, \
    (SELECT group_concat(t.name, char(31) ORDER BY t.name) FROM item_tags it JOIN tags t ON t.id = it.tag_id \
    WHERE it.item_id = i.id) AS tag_list, (SELECT group_concat(c.what, ' ' ORDER BY c.what) FROM confirmations c \
    WHERE c.pkg_id = i.pkg_id) AS confirmed FROM items i LEFT JOIN sources s ON s.id = i.source_id";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Meta {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration_ms: Option<u32>,
    pub audio: Option<bool>,
    pub color: Option<u32>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageInfo {
    pub id: String,
    pub version: String,
    pub kind: String,
    pub license: String,
    pub content: Option<String>,
    pub photosensitive: Option<bool>,
    pub facets: Vec<String>,
    pub confirmed: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub uid: String,
    pub source: Option<String>,
    pub kind: Kind,
    pub path: PathBuf,
    pub title: String,
    pub size: u64,
    pub mtime: i64,
    #[serde(flatten)]
    pub meta: Meta,
    pub package: Option<PackageInfo>,
    pub tags: Vec<String>,
    pub rating: u8,
    pub favorite: bool,
    pub added_at: i64,
    pub used_at: Option<i64>,
    pub use_count: u32,
    pub missing: bool,
}

pub(crate) fn item_row(row: &Row<'_>) -> rusqlite::Result<Item> {
    let words = |column: &str| -> rusqlite::Result<Vec<String>> {
        let text: Option<String> = row.get(column)?;
        Ok(text.iter().flat_map(|t| t.split_whitespace()).map(String::from).collect())
    };
    let package = match row.get::<_, Option<String>>("pkg_id")? {
        Some(id) => Some(PackageInfo {
            id,
            version: row.get("pkg_version")?,
            kind: row.get("pkg_type")?,
            license: row.get("license")?,
            content: row.get("content")?,
            photosensitive: row.get("photosensitive")?,
            facets: words("facets")?,
            confirmed: words("confirmed")?,
        }),
        None => None,
    };
    let tags: Option<String> = row.get("tag_list")?;
    Ok(Item {
        uid: row.get("uid")?,
        source: row.get("source")?,
        kind: row.get("kind")?,
        path: row.get::<_, String>("path")?.into(),
        title: row.get("title")?,
        size: row.get::<_, i64>("size")?.unsigned_abs(),
        mtime: row.get("mtime")?,
        meta: Meta {
            width: row.get("width")?,
            height: row.get("height")?,
            duration_ms: row.get("duration_ms")?,
            audio: row.get("audio")?,
            color: row.get("color")?,
            error: row.get("error")?,
        },
        package,
        tags: tags.map(|t| t.split('\u{1f}').map(String::from).collect()).unwrap_or_default(),
        rating: row.get("rating")?,
        favorite: row.get("favorite")?,
        added_at: row.get("added_at")?,
        used_at: row.get("used_at")?,
        use_count: row.get("use_count")?,
        missing: row.get("missing")?,
    })
}

pub(crate) fn item_in(db: &Connection, uid: &str) -> Result<Item> {
    let sql = format!("{SELECT_ITEMS} WHERE i.uid = ?1");
    db.query_row(&sql, [uid], item_row).optional()?.ok_or_else(|| Error::NotFound("item", uid.into()))
}

impl Library {
    pub fn item(&self, uid: &str) -> Result<Item> {
        item_in(&self.db, uid)
    }

    pub fn set_meta(&mut self, uid: &str, meta: &Meta) -> Result<()> {
        let Meta { width, height, duration_ms, audio, color, error } = meta;
        let sql = "UPDATE items SET width = ?2, height = ?3, duration_ms = ?4, audio = ?5, color = ?6, error = ?7 \
            WHERE uid = ?1";
        let changed = self.db.execute(sql, params![uid, width, height, duration_ms, audio, color, error])?;
        ensure(changed == 1, || Error::NotFound("item", uid.into()))
    }

    pub fn hashed(&self) -> Result<Vec<(Item, Vec<u8>)>> {
        rows(&self.db, SELECT_ITEMS, [], |row| Ok((item_row(row)?, row.get("qhash")?)))
    }

    pub fn unanalyzed(&self, limit: u32) -> Result<Vec<Item>> {
        let sql = format!(
            "{SELECT_ITEMS} WHERE i.missing = 0 AND i.width IS NULL AND i.duration_ms IS NULL AND i.error IS NULL \
            ORDER BY i.added_at, i.id LIMIT ?1"
        );
        rows(&self.db, &sql, [limit], item_row)
    }
}
