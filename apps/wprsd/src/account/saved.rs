use crate::error::{Error, Result, failed};
use crate::store::private_dir;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use wprs_core::model::RepoOffer;
pub use wprs_sync::Peer;
use wprs_sync::{Clock, Replica};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Pair {
    pub device: String,
    pub code: Option<String>,
    pub sharing: bool,
    pub pair: String,
    pub transcript: String,
    pub key: String,
    pub expires: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Stored {
    pub hash: String,
    pub size: u64,
    pub mtime: i64,
    pub done: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Files {
    pub folders: BTreeSet<PathBuf>,
    pub items: BTreeMap<String, bool>,
    pub stored: BTreeMap<String, Stored>,
    pub dropped: BTreeMap<String, String>,
    pub here: BTreeMap<String, String>,
    pub uploaded: BTreeSet<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Saved {
    pub url: String,
    pub name: String,
    pub repo_url: Option<String>,
    pub repo_id: Option<String>,
    pub owner: Option<(String, i64)>,
    pub handle: Option<String>,
    pub device: Option<String>,
    pub device_name: Option<String>,
    pub label: String,
    pub encrypted: Option<bool>,
    pub plaintext: bool,
    pub epoch: i64,
    pub ready: bool,
    pub stale: bool,
    pub paused: bool,
    pub last_sync: Option<i64>,
    pub last_error: Option<String>,
    pub warning: Option<String>,
    pub used_bytes: u64,
    pub quota_bytes: u64,
    pub peers: Vec<Peer>,
    pub pinned: Option<BTreeMap<String, String>>,
    pub members: BTreeSet<String>,
    pub clock: Clock,
    pub replica: Replica,
    pub links: BTreeMap<String, String>,
    pub present: BTreeSet<String>,
    pub names: String,
    pub quarantine: BTreeMap<String, i64>,
    pub legacy: BTreeMap<String, i64>,
    pub pairing: Option<Pair>,
    pub kept: BTreeMap<String, BTreeMap<String, Value>>,
    pub unapplied: BTreeMap<String, i64>,
    pub declined: BTreeSet<String>,
    pub offers: Vec<RepoOffer>,
    pub share: BTreeMap<String, bool>,
    pub shared: BTreeSet<String>,
    pub recovery: Option<i64>,
    pub files: Files,
    pub files_bytes: u64,
    pub files_quota: u64,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub removed: BTreeMap<String, i64>,
}

pub fn load(path: &Path) -> Saved {
    let Ok(bytes) = std::fs::read(path) else { return Saved::default() };
    serde_json::from_slice(&bytes).unwrap_or_else(|e| {
        tracing::warn!("the sync state cannot be read; no server is used until one is added again");
        tracing::debug!("{e}");
        Saved::default()
    })
}

pub fn save(path: &Path, saved: &Saved) -> Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    private_dir(dir)?;
    let mut file = tempfile::Builder::new().prefix(".tmp-").tempfile_in(dir)?;
    file.write_all(&serde_json::to_vec(saved).map_err(|e| failed(e.to_string()))?)?;
    file.persist(path).map_err(|e| Error::from(e.error))?;
    Ok(())
}

impl Saved {
    pub fn active(&self) -> bool {
        !self.url.is_empty() && self.device.is_some() && (self.ready || self.stale) && !self.paused
    }

    pub fn user(&self) -> i64 {
        self.owner.as_ref().map_or(0, |(_, user)| *user)
    }

    pub fn forget(&mut self, wipe: bool) {
        let kept = Saved {
            owner: self.owner.take(),
            clock: std::mem::take(&mut self.clock),
            replica: std::mem::take(&mut self.replica),
            links: std::mem::take(&mut self.links),
            present: std::mem::take(&mut self.present),
            legacy: std::mem::take(&mut self.legacy),
            names: std::mem::take(&mut self.names),
            kept: std::mem::take(&mut self.kept),
            declined: std::mem::take(&mut self.declined),
            files: std::mem::take(&mut self.files),
            ..Saved::default()
        };
        *self = if wipe { Saved::default() } else { kept };
    }
}
