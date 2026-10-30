use crate::batch::{SPAN, batches};
use crate::db::rows;
use crate::probe::{Probe, probe, store};
use crate::scan::skip_file;
use crate::{Kind, Library, Result, ScanReport};
use rusqlite::Connection;
use std::path::Path;
use std::time::Duration;

enum Loose {
    Present(Box<Probe>),
    Gone(i64),
}

impl Library {
    pub fn check_loose(&mut self, now: i64) -> Result<ScanReport> {
        self.check_loose_within(now, SPAN, &mut |_| Ok(()))
    }

    fn check_loose_within(
        &mut self,
        now: i64,
        span: Duration,
        done: &mut dyn FnMut(usize) -> Result<()>,
    ) -> Result<ScanReport> {
        let sql = "SELECT id, path, missing FROM items WHERE source_id IS NULL";
        let loose: Vec<(i64, String, bool)> =
            rows(&self.db, sql, [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        let mut report = ScanReport::default();
        let mut look = |db: &Connection, (id, path, missing): &(i64, String, bool)| {
            let path = Path::new(path);
            Ok(if path.is_file() && Kind::detect(path).is_some() {
                skip_file(probe(db, path, None))?.map(|found| Loose::Present(Box::new(found)))
            } else {
                (!missing).then_some(Loose::Gone(*id))
            })
        };
        let mut keep = |tx: &Connection, found: Loose| {
            match found {
                Loose::Present(mut found) => drop(skip_file(store(tx, None, &mut found, now, &mut report))?),
                Loose::Gone(id) => {
                    tx.execute("UPDATE items SET missing = 1 WHERE id = ?1", [id])?;
                    report.missing += 1;
                }
            }
            Ok(())
        };
        batches(&mut self.db, &loose, span, &mut look, &mut keep, done)?;
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn loose_files_are_read_outside_short_batches_that_write_only_changes() {
        let dir = tempfile::tempdir().unwrap();
        let mut library = Library::open(dir.path().join("library.db")).unwrap();
        let files: Vec<_> = (0..5).map(|n| dir.path().join(format!("{n}.png"))).collect();
        for (n, file) in files.iter().enumerate() {
            fs::write(file, n.to_string()).unwrap();
            library.import_file(file, 1).unwrap();
        }
        let before = library.db.total_changes();
        assert_eq!(library.check_loose(2).unwrap(), ScanReport::default());
        assert_eq!(library.db.total_changes(), before);
        fs::write(&files[1], "changed").unwrap();
        fs::remove_file(&files[3]).unwrap();
        let other = Library::open(dir.path().join("library.db")).unwrap();
        other.db.busy_timeout(Duration::ZERO).unwrap();
        let mut calls = Vec::new();
        let mut record = |done| {
            calls.push((done, other.db.execute_batch("BEGIN IMMEDIATE; COMMIT").is_ok()));
            Ok(())
        };
        let report = library.check_loose_within(3, Duration::ZERO, &mut record).unwrap();
        assert_eq!(report, ScanReport { updated: 1, missing: 1, ..ScanReport::default() });
        assert_eq!(calls, (1..=5).map(|n| (n, true)).collect::<Vec<_>>());
    }
}
