mod apply;
mod cycle;
mod keys;
mod link;
mod local;
mod manage;
mod objects;
mod pending;
mod presets;
mod saved;
mod secrets;
mod server;
mod share;
mod status;
mod upload;
mod views;
mod watch;

pub use watch::start;

use crate::compositor::lock;
use crate::daemon::Daemon;
use crate::error::{Error, Result, failed, invalid, not_found};
use crate::keyring::Keyring;
use saved::{Peer, Saved};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};
use tokio::sync::Notify;
use wprs_core::model::JobKind;
use wprs_sync::Api;

pub const KINDS: [JobKind; 4] = [JobKind::Account, JobKind::Link, JobKind::AccountSync, JobKind::Upload];

#[derive(Default)]
pub struct Account {
    saved: Mutex<Saved>,
    keyring: OnceLock<Keyring>,
    wake: Notify,
    restart: Notify,
}

pub fn block_on<T>(work: impl Future<Output = T>) -> T {
    tokio::runtime::Handle::current().block_on(work)
}

pub fn sync_error(e: wprs_sync::Error) -> Error {
    match e {
        wprs_sync::Error::Invalid(message) => invalid(message),
        e => failed(e.to_string()),
    }
}

fn millis(value: &Value) -> Option<i64> {
    value.as_i64().map(|ms| ms / 1000)
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
        let found = block_on(api.get("/api/v1/me/devices")).map_err(sync_error)?;
        let peer = |d: &Value| Peer {
            id: d["id"].as_str().unwrap_or_default().into(),
            name: d["name"].as_str().unwrap_or_default().into(),
            platform: d["platform"].as_str().unwrap_or_default().into(),
            enc_key: d["enc_key"].as_str().map(String::from),
            created_at: millis(&d["created_at"]).unwrap_or_default(),
            seen_at: millis(&d["seen_at"]),
        };
        let peers: Vec<Peer> = found["items"].as_array().into_iter().flatten().map(peer).collect();
        self.update(|s| s.peers = peers.clone())?;
        Ok(peers)
    }
}

fn hostname() -> String {
    let name = rustix::system::uname().nodename().to_string_lossy().into_owned();
    if name.trim().is_empty() { "Linux computer".into() } else { name }
}
