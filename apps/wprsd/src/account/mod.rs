mod apply;
mod cycle;
mod escrow;
mod fetch;
mod files;
mod gone;
mod keys;
mod legacy;
mod link;
mod local;
mod mail;
mod manage;
mod objects;
mod offers;
mod pending;
mod presets;
mod rotate;
mod saved;
mod secrets;
mod server;
mod share;
mod shared;
mod status;
#[cfg(test)]
mod tests;
mod transfer;
mod upload;
mod watch;

pub use watch::start;
pub use wprs_sync::DOWNLOADS;
pub use wprs_sync::{removed_blob, removed_package};

use crate::compositor::lock;
use crate::daemon::Daemon;
use crate::error::{Error, Result, failed, not_found};
use crate::keyring::Keyring;
use saved::{Peer, Saved};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::{Mutex, MutexGuard, OnceLock};
use tokio::sync::Notify;
use wprs_core::model::JobKind;
use wprs_sync::{Api, devices};

pub const KINDS: [JobKind; 4] = [JobKind::Account, JobKind::Link, JobKind::AccountSync, JobKind::Upload];

#[derive(Default)]
pub struct Account {
    saved: Mutex<Saved>,
    keyring: OnceLock<Keyring>,
    wake: Notify,
    restart: Notify,
    confirmed: Mutex<Option<(String, String)>>,
    edits: AtomicBool,
    tries: AtomicU32,
}

pub fn block_on<T>(work: impl Future<Output = T>) -> T {
    tokio::runtime::Handle::current().block_on(work)
}

pub fn sync_error(e: wprs_sync::Error) -> Error {
    match e.code() == wprs_sync::TOO_OLD {
        true => failed(crate::words::too_old()),
        false => e.into(),
    }
}

impl Daemon {
    fn saved(&self) -> MutexGuard<'_, Saved> {
        lock(&self.account.saved)
    }

    fn sync_path(&self) -> PathBuf {
        self.store.data.join("sync.json")
    }

    fn update(&self, change: impl FnOnce(&mut Saved)) -> Result<()> {
        let mut saved = self.saved();
        change(&mut saved);
        saved::save(&self.sync_path(), &saved)?;
        drop(saved);
        self.changes.changed("account", Vec::new());
        Ok(())
    }

    fn jobs_of(&self, kind: JobKind) -> Vec<u64> {
        let jobs = self.jobs.list().into_iter().filter(|j| j.kind == kind && j.finished_at.is_none());
        jobs.map(|j| j.id).collect()
    }

    fn server_url(&self) -> Result<String> {
        let url = self.saved().url.clone();
        if url.is_empty() { Err(not_found("no server is added; add one first")) } else { Ok(url) }
    }

    fn linked(&self) -> Result<()> {
        self.server_url()?;
        let linked = self.saved().device.is_some();
        if linked { Ok(()) } else { Err(failed("link this computer to the server first")) }
    }

    fn refresh_peers(&self, api: &Api) -> Result<Vec<Peer>> {
        let peers = block_on(devices(api)).map_err(sync_error)?;
        self.update(|s| {
            s.pinned.iter_mut().for_each(|pins| pins.retain(|id, _| peers.iter().any(|p| p.id == *id)));
            s.members.extend(peers.iter().map(|p| p.id.clone()));
            s.peers = peers.clone();
        })?;
        Ok(peers)
    }

    fn pins(&self) -> Result<BTreeMap<String, String>> {
        let seeded = {
            let s = self.saved();
            if let Some(pins) = &s.pinned {
                return Ok(pins.clone());
            }
            let record =
                |id: &str| s.replica.bases.get(&format!("device/{id}")).and_then(|(_, doc)| doc.view());
            let vouched = |p: &&Peer| {
                record(&p.id).and_then(|v| v.get("key").cloned()) == p.enc_key.clone().map(Value::from)
            };
            let peers = s.peers.iter().filter(|p| Some(&p.id) != s.device.as_ref()).filter(vouched);
            peers.filter_map(|p| Some((p.id.clone(), p.enc_key.clone()?))).collect::<BTreeMap<_, _>>()
        };
        self.update(|s| s.pinned = Some(seeded.clone()))?;
        Ok(seeded)
    }

    fn pin(&self, peer: &str, key: &str) -> Result<()> {
        self.update(|s| drop(s.pinned.get_or_insert_default().insert(peer.to_owned(), key.to_owned())))
    }

    fn rekeyed(&self, api: &Api) -> Result<bool> {
        let before = self.saved().members.clone();
        let peers = self.refresh_peers(api)?;
        let left = before.iter().any(|id| !peers.iter().any(|p| p.id == *id));
        self.update(|s| s.members = peers.iter().map(|p| p.id.clone()).collect())?;
        Ok(left)
    }
}

fn hostname() -> String {
    let name = rustix::system::uname().nodename().to_string_lossy().into_owned();
    if name.trim().is_empty() { "Linux computer".into() } else { name }
}
