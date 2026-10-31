use crate::history::History;
use crate::project::{Project, write};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use wprs_format::sha256;

#[derive(Serialize, Deserialize)]
pub(crate) struct Saved {
    pub files: String,
    pub doc: Value,
    pub history: History,
}

const KEPT: usize = 32;
const KEEP_FOR: Duration = Duration::from_secs(30 * 86_400);

fn sweep(dir: &Path, keep: &Path) {
    let old = SystemTime::now().checked_sub(KEEP_FOR).unwrap_or(UNIX_EPOCH);
    let entries = fs::read_dir(dir).into_iter().flatten().flatten();
    let mut found: Vec<(SystemTime, PathBuf)> = entries
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .filter(|(_, p)| p != keep && p.extension().is_some_and(|e| e == "json"))
        .collect();
    found.sort();
    let excess = found.len().saturating_sub(KEPT - 1);
    for (i, (modified, path)) in found.iter().enumerate() {
        if i < excess || *modified < old {
            fs::remove_file(path).ok();
        }
    }
}

fn state(dir: &Path) -> Result<PathBuf> {
    let dir = fs::canonicalize(dir)?;
    let name = sha256(dir.as_os_str().as_encoded_bytes())?;
    let base = dirs::state_dir().context("there is no state folder")?;
    Ok(base.join("wallpaper-rs").join("editor").join(format!("{}.json", &name[..32])))
}

pub(crate) fn saved(dir: &Path) -> Option<Saved> {
    serde_json::from_slice(&fs::read(state(dir).ok()?).ok()?).ok()
}

impl Project {
    pub fn persist(&self) -> Result<()> {
        let path = state(&self.dir)?;
        if self.pending.is_none()
            && !self.dirty()
            && self.history.done.is_empty()
            && self.history.undone.is_empty()
        {
            return self.forget();
        }
        let dir = path.parent().context("a file needs a folder")?;
        fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
        let saved = Saved { files: self.base.clone(), doc: self.doc.clone(), history: self.history.clone() };
        write(&path, &serde_json::to_string(&saved)?, 0o600)?;
        sweep(dir, &path);
        Ok(())
    }

    pub fn forget(&self) -> Result<()> {
        match fs::remove_file(state(&self.dir)?) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
            _ => Ok(()),
        }
    }

    pub fn recover(&mut self, keep: bool) -> Result<()> {
        let Some((doc, history)) = self.pending.take() else { return Ok(()) };
        if keep {
            (self.doc, self.history) = (doc, history);
        }
        self.persist()
    }
}
