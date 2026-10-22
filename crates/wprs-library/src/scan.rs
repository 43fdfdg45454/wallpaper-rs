use crate::db::{ensure, remove, row_id, rows, uid, unique};
use crate::import::{upsert, utf8};
use crate::{Error, Kind, Library, Result};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    pub uid: String,
    pub path: PathBuf,
    pub recursive: bool,
    pub scanned_at: Option<i64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanReport {
    pub added: u32,
    pub updated: u32,
    pub moved: u32,
    pub missing: u32,
}

impl Library {
    pub fn add_folder(&mut self, path: impl AsRef<Path>, recursive: bool) -> Result<String> {
        let path = fs::canonicalize(path)?;
        ensure(path.is_dir(), || Error::Invalid("folder", format!("{} is not a directory", path.display())))?;
        let (uid, uri) = (uid(), utf8(&path)?);
        let sql = "INSERT INTO sources(uid, uri, recursive) VALUES (?1, ?2, ?3)";
        unique(self.db.execute(sql, params![uid, uri, recursive]), "source", uri)?;
        Ok(uid)
    }

    pub fn sources(&self) -> Result<Vec<Source>> {
        let sql = "SELECT uid, uri, recursive, scanned_at, error FROM sources ORDER BY uri";
        rows(&self.db, sql, [], |r| {
            let (uid, path) = (r.get(0)?, r.get::<_, String>(1)?.into());
            Ok(Source { uid, path, recursive: r.get(2)?, scanned_at: r.get(3)?, error: r.get(4)? })
        })
    }

    pub fn remove_source(&mut self, uid: &str) -> Result<()> {
        remove(&self.db, "source", uid)
    }

    pub fn scan(&mut self, uid: &str, now: i64) -> Result<ScanReport> {
        let tx = self.db.transaction()?;
        let id = row_id(&tx, "source", uid)?;
        let sql = "SELECT uri, recursive FROM sources WHERE id = ?1";
        let (root, recursive): (String, bool) = tx.query_row(sql, [id], |r| Ok((r.get(0)?, r.get(1)?)))?;
        let mut files = Vec::new();
        let walked = walk(Path::new(&root), recursive, &mut files);
        let error = walked.as_ref().err().map(ToString::to_string);
        tx.execute("UPDATE sources SET scanned_at = ?2, error = ?3 WHERE id = ?1", params![id, now, error])?;
        if let Err(e) = walked {
            tx.commit()?;
            return Err(e.into());
        }
        files.sort();
        let (mut report, mut seen) = (ScanReport::default(), HashSet::new());
        for file in files {
            match upsert(&tx, Some(id), &file, now, &mut report) {
                Ok(item) => _ = seen.insert(item),
                Err(Error::Io(_)) => {}
                Err(e) => return Err(e),
            }
        }
        let sql = "SELECT id FROM items WHERE source_id = ?1 AND missing = 0";
        let present: Vec<i64> = rows(&tx, sql, [id], |r| r.get(0))?;
        for item in present.into_iter().filter(|item| !seen.contains(item)) {
            tx.execute("UPDATE items SET missing = 1 WHERE id = ?1", [item])?;
            report.missing += 1;
        }
        tx.commit()?;
        Ok(report)
    }
}

fn walk(dir: &Path, recursive: bool, files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)?.flatten().filter(|e| !e.file_name().to_string_lossy().starts_with('.')) {
        let path = entry.path();
        match entry.file_type() {
            Ok(kind) if kind.is_dir() && recursive => _ = walk(&path, true, files),
            _ if Kind::detect(&path).is_some() && path.is_file() => files.push(path),
            _ => {}
        }
    }
    Ok(())
}
