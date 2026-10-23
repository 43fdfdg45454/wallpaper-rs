use crate::db::{ensure, row_id, rows, uid, unique};
use crate::packages::package;
use crate::{Error, Kind, Library, Result, ScanReport};
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom};
use std::os::unix::fs::MetadataExt;
use std::path::Path;

const MIB: u64 = 1 << 20;

pub fn qhash(path: impl AsRef<Path>) -> io::Result<[u8; 32]> {
    let mut file = File::open(path)?;
    let size = file.metadata()?.len();
    let (mut hasher, mut chunk) = (Sha256::new(), Vec::new());
    hasher.update(size.to_le_bytes());
    for start in [0, MIB.max(size.saturating_sub(MIB))] {
        chunk.clear();
        file.seek(SeekFrom::Start(start))?;
        (&mut file).take(MIB).read_to_end(&mut chunk)?;
        hasher.update(&chunk);
    }
    Ok(hasher.finalize().into())
}

impl Library {
    pub fn import_file(&mut self, path: impl AsRef<Path>, now: i64) -> Result<String> {
        let path = fs::canonicalize(path)?;
        let tx = self.db.transaction()?;
        let id = upsert(&tx, None, &path, now, &mut ScanReport::default())?;
        let uid = tx.query_row("SELECT uid FROM items WHERE id = ?1", [id], |r| r.get(0))?;
        tx.commit()?;
        Ok(uid)
    }

    pub fn relink_item(&mut self, uid: &str, path: &Path, _now: i64) -> Result<()> {
        let path = fs::canonicalize(path)?;
        let tx = self.db.transaction()?;
        let (id, text) = (row_id(&tx, "item", uid)?, utf8(&path)?);
        let sql = "UPDATE items SET path = ?2, missing = 0 WHERE id = ?1";
        unique(tx.execute(sql, params![id, text]), "item", text)?;
        refresh(&tx, id, &path)?;
        Ok(tx.commit()?)
    }
}

fn facts(path: &Path) -> Result<(Kind, &str, i64, i64)> {
    let not_media =
        || Error::Invalid("file", format!("{} is not an image, video or package", path.display()));
    let kind = Kind::detect(path).ok_or_else(not_media)?;
    let (text, meta) = (utf8(path)?, fs::metadata(path)?);
    ensure(meta.is_file(), not_media)?;
    Ok((kind, text, meta.len() as i64, meta.mtime()))
}

pub(crate) fn upsert(
    db: &Connection,
    source: Option<i64>,
    path: &Path,
    now: i64,
    report: &mut ScanReport,
) -> Result<i64> {
    let (kind, text, size, mtime) = facts(path)?;
    let sql = "SELECT id, size = ?2 AND mtime = ?3, missing FROM items WHERE path = ?1";
    let known: Option<(i64, bool, bool)> = db
        .query_row(sql, params![text, size, mtime], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .optional()?;
    let found = "UPDATE items SET path = ?2, mtime = ?3, missing = 0, source_id = coalesce(?4, source_id) WHERE id = ?1";
    if let Some((id, same, missing)) = known {
        db.execute(found, params![id, text, mtime, source])?;
        if !same {
            refresh(db, id, path)?;
        }
        report.updated += u32::from(missing || !same);
        return Ok(id);
    }
    let hash = qhash(path)?;
    let sql = "SELECT id, path, missing FROM items WHERE qhash = ?1";
    let candidates: Vec<(i64, String, bool)> =
        rows(db, sql, [hash], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    if let Some((id, ..)) =
        candidates.into_iter().find(|(_, old, missing)| *missing || !Path::new(old).exists())
    {
        db.execute(found, params![id, text, mtime, source])?;
        report.moved += 1;
        return Ok(id);
    }
    let title = path.file_stem().map(|stem| stem.to_string_lossy().into_owned()).unwrap_or_default();
    let sql = "INSERT INTO items(uid, source_id, kind, path, title, size, mtime, qhash, added_at) \
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)";
    db.execute(sql, params![uid(), source, kind, text, title, size, mtime, hash, now])?;
    let id = db.last_insert_rowid();
    if let Some(manifest) = package(db, id, kind, path)? {
        let title = manifest.title_in(&wprs_core::locale::languages());
        db.execute("UPDATE items SET title = ?2 WHERE id = ?1", params![id, title])?;
    }
    report.added += 1;
    Ok(id)
}

fn refresh(db: &Connection, id: i64, path: &Path) -> Result<()> {
    let (kind, _, size, mtime) = facts(path)?;
    let sql = "UPDATE items SET kind = ?2, size = ?3, mtime = ?4, qhash = ?5, width = NULL, height = NULL, \
        duration_ms = NULL, audio = NULL, color = NULL, error = NULL, pkg_id = NULL, pkg_version = NULL, \
        pkg_type = NULL, license = NULL, content = NULL, photosensitive = NULL, facets = NULL WHERE id = ?1";
    db.execute(sql, params![id, kind, size, mtime, qhash(path)?])?;
    package(db, id, kind, path).map(drop)
}

pub(crate) fn utf8(path: &Path) -> Result<&str> {
    path.to_str().ok_or_else(|| Error::Invalid("path", format!("{} is not valid UTF-8", path.display())))
}
