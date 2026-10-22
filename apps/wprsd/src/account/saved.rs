use crate::error::{Error, Result, failed};
use crate::store::private_dir;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::Path;
use wprs_sync::{Clock, Replica};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Peer {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub enc_key: Option<String>,
    pub created_at: i64,
    pub seen_at: Option<i64>,
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
    pub epoch: i64,
    pub ready: bool,
    pub paused: bool,
    pub last_sync: Option<i64>,
    pub last_error: Option<String>,
    pub warning: Option<String>,
    pub used_bytes: u64,
    pub quota_bytes: u64,
    pub peers: Vec<Peer>,
    pub clock: Clock,
    pub replica: Replica,
    pub links: BTreeMap<String, String>,
    pub present: BTreeSet<String>,
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
        !self.url.is_empty() && self.device.is_some() && self.ready && !self.paused
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
            ..Saved::default()
        };
        *self = if wipe { Saved::default() } else { kept };
    }
}
