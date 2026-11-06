use crate::history::History;
use crate::project::Project;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use wprs_core::message::said;
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
            fs::remove_dir_all(path.with_extension("meshes")).ok();
        }
    }
}

fn state(dir: &Path) -> Result<PathBuf> {
    let dir = fs::canonicalize(dir)?;
    let name = sha256(dir.as_os_str().as_encoded_bytes())?;
    let base =
        dirs::state_dir().ok_or_else(|| said("editor.no_state_folder", "there is no state folder", &[]))?;
    Ok(base.join("wallpaper-rs").join("editor").join(format!("{}.json", &name[..32])))
}

pub(crate) fn stash(dir: &Path) -> Result<PathBuf> {
    Ok(state(dir)?.with_extension("meshes"))
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
        let dir = path.parent().ok_or_else(|| said("editor.needs_folder", "a file needs a folder", &[]))?;
        fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
        let saved = Saved { files: self.base.clone(), doc: self.doc.clone(), history: self.history.clone() };
        write(&path, &serde_json::to_string(&saved)?, 0o600)?;
        sweep(dir, &path);
        Ok(())
    }

    pub fn forget(&self) -> Result<()> {
        if let Ok(stash) = stash(&self.dir) {
            fs::remove_dir_all(stash).ok();
        }
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
        self.bring_back();
        self.persist()
    }

    pub(crate) fn ready(&self) -> Result<()> {
        anyhow::ensure!(
            self.pending.is_none(),
            said(
                "editor.unsaved_changes_earlier",
                "unsaved changes of an earlier session wait; recover or discard them first",
                &[]
            )
        );
        Ok(())
    }
}

pub(crate) fn write(path: &Path, text: &str, mode: u32) -> Result<()> {
    let dir = path.parent().ok_or_else(|| said("editor.needs_folder", "a file needs a folder", &[]))?;
    let tmp = dir.join(format!(".{}.tmp", path.file_name().unwrap_or_default().to_string_lossy()));
    let mut file = fs::OpenOptions::new().write(true).create(true).truncate(true).mode(mode).open(&tmp)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    Ok(fs::rename(tmp, path)?)
}
