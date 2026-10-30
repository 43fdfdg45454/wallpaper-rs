use crate::db::{row_id, unique};
use crate::probe::{probe, refresh, store};
use crate::{Error, Library, Result, ScanReport};
use rusqlite::params;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom};
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
        let mut found = probe(&self.db, &path, None)?;
        let tx = self.db.transaction()?;
        let id = store(&tx, None, &mut found, now, &mut ScanReport::default())?;
        let uid = tx.query_row("SELECT uid FROM items WHERE id = ?1", [id], |r| r.get(0))?;
        tx.commit()?;
        Ok(uid)
    }

    pub fn relink_item(&mut self, uid: &str, path: &Path, _now: i64) -> Result<()> {
        let path = fs::canonicalize(path)?;
        let mut found = probe(&self.db, &path, None)?;
        found.fill()?;
        let tx = self.db.transaction()?;
        let (id, text) = (row_id(&tx, "item", uid)?, utf8(&path)?);
        let sql = "UPDATE items SET path = ?2, missing = 0 WHERE id = ?1";
        unique(tx.execute(sql, params![id, text]), "item", text)?;
        refresh(&tx, id, &mut found)?;
        Ok(tx.commit()?)
    }
}

pub(crate) fn utf8(path: &Path) -> Result<&str> {
    path.to_str().ok_or_else(|| Error::Invalid("path", format!("{} is not valid UTF-8", path.display())))
}
