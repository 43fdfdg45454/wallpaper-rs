use crate::db::{ensure, printable, row_id, rows, uid, unique};
use crate::tags::{TAG_LENGTH, tag_item};
use crate::{Error, Kind, Library, Result, ScanReport};
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom};
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use wprs_format::{Limits, Manifest};

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
        db.execute("UPDATE items SET title = ?2 WHERE id = ?1", params![id, manifest.title])?;
    }
    report.added += 1;
    Ok(id)
}

fn refresh(db: &Connection, id: i64, path: &Path) -> Result<()> {
    let (kind, _, size, mtime) = facts(path)?;
    let sql = "UPDATE items SET kind = ?2, size = ?3, mtime = ?4, qhash = ?5, width = NULL, height = NULL, \
        duration_ms = NULL, audio = NULL, color = NULL, error = NULL, pkg_id = NULL, pkg_version = NULL, \
        pkg_type = NULL, license = NULL WHERE id = ?1";
    db.execute(sql, params![id, kind, size, mtime, qhash(path)?])?;
    package(db, id, kind, path).map(drop)
}

fn package(db: &Connection, id: i64, kind: Kind, path: &Path) -> Result<Option<Manifest>> {
    if kind != Kind::Package {
        return Ok(None);
    }
    let (manifest, error) = match wprs_format::open(path, &Limits::default()) {
        Ok(package) => (Some(package.manifest), None),
        Err(e) => (None, Some(e.to_string())),
    };
    let m = manifest.as_ref();
    let (pkg_id, version) = (m.map(|m| &m.id), m.map(|m| &m.version));
    let (pkg_type, license) = (m.map(|m| m.kind.to_string()), m.map(|m| &m.license));
    let sql = "UPDATE items SET error = ?2, pkg_id = ?3, pkg_version = ?4, pkg_type = ?5, license = ?6 WHERE id = ?1";
    db.execute(sql, params![id, error, pkg_id, version, pkg_type, license])?;
    for tag in m.iter().flat_map(|m| &m.tags).filter(|tag| printable(tag.trim(), TAG_LENGTH)) {
        tag_item(db, id, tag)?;
    }
    Ok(manifest)
}

pub(crate) fn utf8(path: &Path) -> Result<&str> {
    path.to_str().ok_or_else(|| Error::Invalid("path", format!("{} is not valid UTF-8", path.display())))
}
