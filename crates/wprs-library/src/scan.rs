use crate::db::{ensure, row_id, rows, uid, unique};
use crate::import::{upsert, utf8};
use crate::{Error, Kind, Library, Result};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const BATCH: usize = 256;
const SPAN: Duration = Duration::from_secs(1);

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

    pub fn scan(&mut self, uid: &str, now: i64) -> Result<ScanReport> {
        self.scan_with(uid, now, &mut |_, _| ControlFlow::Continue(()))
    }

    pub fn scan_with(
        &mut self,
        uid: &str,
        now: i64,
        progress: &mut dyn FnMut(u32, u32) -> ControlFlow<()>,
    ) -> Result<ScanReport> {
        self.scan_within(uid, now, progress, SPAN)
    }

    fn scan_within(
        &mut self,
        uid: &str,
        now: i64,
        progress: &mut dyn FnMut(u32, u32) -> ControlFlow<()>,
        span: Duration,
    ) -> Result<ScanReport> {
        let id = row_id(&self.db, "source", uid)?;
        let sql = "SELECT uri, recursive FROM sources WHERE id = ?1";
        let (root, recursive): (String, bool) =
            self.db.query_row(sql, [id], |r| Ok((r.get(0)?, r.get(1)?)))?;
        let mut files = Vec::new();
        if let Err(e) = walk(Path::new(&root), recursive, &mut files) {
            let sql = "UPDATE sources SET scanned_at = ?2, error = ?3 WHERE id = ?1";
            self.db.execute(sql, params![id, now, e.to_string()])?;
            return Err(e.into());
        }
        files.sort();
        let (mut report, mut seen, total) = (ScanReport::default(), HashSet::new(), files.len() as u32);
        let source = row_id(&self.db, "source", uid)?;
        let mut store = |tx: &Connection, file: &PathBuf| {
            seen.extend(skip_file(upsert(tx, Some(source), file, now, &mut report))?);
            Ok(())
        };
        batches(&mut self.db, &files, span, &mut store, &mut |done| {
            ensure(progress(done as u32, total).is_continue(), || Error::Cancelled)
        })?;
        let tx = self.db.transaction()?;
        let id = row_id(&tx, "source", uid)?;
        tx.execute("UPDATE sources SET scanned_at = ?2, error = NULL WHERE id = ?1", params![id, now])?;
        let sql = "SELECT id FROM items WHERE source_id = ?1 AND missing = 0";
        let present: Vec<i64> = rows(&tx, sql, [id], |r| r.get(0))?;
        for item in present.into_iter().filter(|item| !seen.contains(item)) {
            tx.execute("UPDATE items SET missing = 1 WHERE id = ?1", [item])?;
            report.missing += 1;
        }
        tx.commit()?;
        Ok(report)
    }

    pub fn check_loose(&mut self, now: i64) -> Result<ScanReport> {
        let sql = "SELECT id, path, missing FROM items WHERE source_id IS NULL";
        let loose: Vec<(i64, String, bool)> =
            rows(&self.db, sql, [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        let mut report = ScanReport::default();
        let mut check = |tx: &Connection, (id, path, missing): &(i64, String, bool)| {
            let path = Path::new(path);
            if path.is_file() && Kind::detect(path).is_some() {
                skip_file(upsert(tx, None, path, now, &mut report))?;
            } else if !missing {
                tx.execute("UPDATE items SET missing = 1 WHERE id = ?1", [id])?;
                report.missing += 1;
            }
            Ok(())
        };
        batches(&mut self.db, &loose, SPAN, &mut check, &mut |_| Ok(()))?;
        Ok(report)
    }
}

fn batches<T>(
    db: &mut Connection,
    list: &[T],
    span: Duration,
    work: &mut dyn FnMut(&Connection, &T) -> Result<()>,
    done: &mut dyn FnMut(usize) -> Result<()>,
) -> Result<()> {
    let mut finished = 0;
    while finished < list.len() {
        let (tx, started) = (db.transaction()?, Instant::now());
        for item in list[finished..].iter().take(BATCH) {
            work(&tx, item)?;
            finished += 1;
            if started.elapsed() >= span {
                break;
            }
        }
        tx.commit()?;
        done(finished)?;
    }
    Ok(())
}

fn skip_file<T>(result: Result<T>) -> Result<Option<T>> {
    match result {
        Err(Error::Io(_) | Error::Invalid(..)) => Ok(None),
        other => other.map(Some),
    }
}

fn walk(dir: &Path, recursive: bool, files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)?.flatten().filter(|e| !e.file_name().to_string_lossy().starts_with('.')) {
        let path = entry.path();
        match entry.file_type() {
            Ok(kind) if kind.is_dir() && recursive => _ = walk(&path, true, files),
            _ if Kind::detect(&path).is_some() && path.to_str().is_some() && path.is_file() => {
                files.push(path)
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slow_batch_is_committed_before_its_time_runs_out() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("pictures");
        fs::create_dir_all(&root).unwrap();
        (0..5).for_each(|n| fs::write(root.join(format!("{n}.png")), n.to_string()).unwrap());
        let mut library = Library::open(dir.path().join("library.db")).unwrap();
        let source = library.add_folder(&root, false).unwrap();
        let other = Library::open(dir.path().join("library.db")).unwrap();
        other.db.busy_timeout(Duration::ZERO).unwrap();
        let mut calls = Vec::new();
        let mut record = |done, _| {
            calls.push((done, other.db.execute_batch("BEGIN IMMEDIATE; COMMIT").is_ok()));
            ControlFlow::Continue(())
        };
        library.scan_within(&source, 1, &mut record, Duration::ZERO).unwrap();
        assert_eq!(calls, (1..=5).map(|n| (n, true)).collect::<Vec<_>>());
    }
}
