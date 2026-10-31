use crate::daemon::Daemon;
use crate::error::{Result, invalid, not_found};
use serde::Serialize;
use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

const FILE: &str = "revealed.json";
static WRITING: Mutex<()> = Mutex::new(());

pub fn save(dir: &Path, file: &str, value: &impl Serialize) -> std::io::Result<()> {
    let mut f = tempfile::Builder::new().prefix(".tmp-").tempfile_in(dir)?;
    f.write_all(&serde_json::to_vec(value).unwrap_or_default())?;
    f.persist(dir.join(file)).map(drop).map_err(|e| e.error)
}

impl Daemon {
    pub fn revealed(&self) -> BTreeSet<String> {
        let bytes = std::fs::read(self.store.data.join(FILE)).unwrap_or_default();
        serde_json::from_slice(&bytes).unwrap_or_default()
    }

    pub fn reveal(&self, spec: &str) -> Result<()> {
        let (repo, rest) = spec.rsplit_once('/').unwrap_or(("", spec));
        let id = rest.split('@').next().unwrap_or(rest);
        let found = wprs_format::browse(&self.store.data, "")?;
        let mut found = found.into_iter().filter(|i| i.id == id && i.repo_id.starts_with(repo));
        let item = found.next().ok_or_else(|| not_found(format!("package {spec:?} not found")))?;
        if found.next().is_some() {
            return Err(invalid("several repositories offer this package; prefix it with the repository id"));
        }
        self.reveals(|set| set.insert(format!("{}/{id}", item.repo_id)))
    }

    pub fn reveals(&self, edit: impl FnOnce(&mut BTreeSet<String>) -> bool) -> Result<()> {
        let _writing = WRITING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut set = self.revealed();
        if edit(&mut set) {
            save(&self.store.data, FILE, &set)?;
            self.changes.changed("repos", Vec::new());
        }
        Ok(())
    }
}
