use crate::db::{ensure, rows, uid};
use crate::import::{qhash, utf8};
use crate::packages::{Reading, read_package, store_package};
use crate::{Error, Kind, Result, ScanReport};
use rusqlite::{Connection, OptionalExtension, params};
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

pub(crate) struct Probe {
    path: PathBuf,
    kind: Kind,
    size: i64,
    mtime: i64,
    hash: Option<[u8; 32]>,
    reading: Option<Reading>,
}

type Known = Option<(i64, bool, bool, bool)>;

pub(crate) fn probe(db: &Connection, path: &Path, source: Option<i64>) -> Result<Probe> {
    let not_media =
        || Error::Invalid("file", format!("{} is not an image, video or package", path.display()));
    let kind = Kind::detect(path).ok_or_else(not_media)?;
    let meta = fs::metadata(path)?;
    ensure(meta.is_file(), not_media)?;
    let (size, mtime) = (meta.len() as i64, meta.mtime());
    let mut probe = Probe { path: path.to_owned(), kind, size, mtime, hash: None, reading: None };
    if !probe.known(db, source)?.is_some_and(|(_, same, ..)| same) {
        probe.fill()?;
    }
    Ok(probe)
}

impl Probe {
    pub(crate) fn fill(&mut self) -> Result<()> {
        if self.hash.is_none() {
            self.hash = Some(qhash(&self.path)?);
            self.reading = read_package(self.kind, &self.path);
        }
        Ok(())
    }

    fn known(&self, db: &Connection, source: Option<i64>) -> Result<Known> {
        let sql = "SELECT id, size = ?2 AND mtime = ?3, missing, ?4 IS NULL OR source_id IS ?4 FROM items WHERE path = ?1";
        let values = params![utf8(&self.path)?, self.size, self.mtime, source];
        Ok(db.query_row(sql, values, |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).optional()?)
    }
}

pub(crate) fn store(
    db: &Connection,
    source: Option<i64>,
    probe: &mut Probe,
    now: i64,
    report: &mut ScanReport,
) -> Result<i64> {
    let (text, mtime) = (utf8(&probe.path)?.to_owned(), probe.mtime);
    let found = "UPDATE items SET path = ?2, mtime = ?3, missing = 0, source_id = coalesce(?4, source_id) WHERE id = ?1";
    if let Some((id, same, missing, owned)) = probe.known(db, source)? {
        if !(same && !missing && owned) {
            db.execute(found, params![id, text, mtime, source])?;
        }
        if !same {
            refresh(db, id, probe)?;
        }
        report.updated += u32::from(missing || !same);
        return Ok(id);
    }
    probe.fill()?;
    let sql = "SELECT id, path, missing FROM items WHERE qhash = ?1";
    let candidates: Vec<(i64, String, bool)> =
        rows(db, sql, [probe.hash], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    if let Some((id, ..)) =
        candidates.into_iter().find(|(_, old, missing)| *missing || !Path::new(old).exists())
    {
        db.execute(found, params![id, text, mtime, source])?;
        report.moved += 1;
        return Ok(id);
    }
    let title = probe.path.file_stem().map(|stem| stem.to_string_lossy().into_owned()).unwrap_or_default();
    let sql = "INSERT INTO items(uid, source_id, kind, path, title, size, mtime, qhash, added_at) \
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)";
    db.execute(sql, params![uid(), source, probe.kind, text, title, probe.size, mtime, probe.hash, now])?;
    let id = db.last_insert_rowid();
    if let Some(reading) = &probe.reading {
        store_package(db, id, reading)?;
        if let Some(manifest) = &reading.manifest {
            let title = manifest.title_in(&wprs_core::locale::languages());
            db.execute("UPDATE items SET title = ?2 WHERE id = ?1", params![id, title])?;
        }
    }
    report.added += 1;
    Ok(id)
}

pub(crate) fn refresh(db: &Connection, id: i64, probe: &mut Probe) -> Result<()> {
    probe.fill()?;
    let sql = "UPDATE items SET kind = ?2, size = ?3, mtime = ?4, qhash = ?5, width = NULL, height = NULL, \
        duration_ms = NULL, audio = NULL, color = NULL, error = NULL, pkg_id = NULL, pkg_version = NULL, \
        pkg_type = NULL, license = NULL, content = NULL, photosensitive = NULL, facets = NULL, base = NULL WHERE id = ?1";
    db.execute(sql, params![id, probe.kind, probe.size, probe.mtime, probe.hash])?;
    probe.reading.as_ref().map_or(Ok(()), |reading| store_package(db, id, reading))
}
