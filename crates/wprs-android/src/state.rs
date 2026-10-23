use crate::{Error, link, store, vault};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};
use wprs_sync::{Api, Clock, Kept, Peer, Replica, Secrets};

const FILE: &str = "sync.json";

static SAVED: Mutex<Option<Saved>> = Mutex::new(None);
pub(crate) static CANCEL: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Saved {
    pub(crate) url: String,
    pub(crate) name: String,
    pub(crate) owner: Option<(String, i64)>,
    pub(crate) handle: Option<String>,
    pub(crate) device: Option<String>,
    pub(crate) device_name: Option<String>,
    pub(crate) label: String,
    pub(crate) encrypted: Option<bool>,
    pub(crate) plaintext: bool,
    pub(crate) epoch: i64,
    pub(crate) ready: bool,
    pub(crate) stale: bool,
    pub(crate) paused: bool,
    pub(crate) last_sync: Option<i64>,
    pub(crate) last_error: Option<String>,
    pub(crate) warning: Option<String>,
    pub(crate) peers: Vec<Peer>,
    pub(crate) pinned: Option<BTreeMap<String, String>>,
    pub(crate) members: BTreeSet<String>,
    pub(crate) clock: Clock,
    pub(crate) replica: Replica,
    pub(crate) links: BTreeMap<String, String>,
    pub(crate) present: BTreeSet<String>,
    pub(crate) names: String,
    pub(crate) quarantine: BTreeMap<String, i64>,
    pub(crate) kept: Kept,
    pub(crate) unapplied: BTreeMap<String, i64>,
}

impl Saved {
    pub(crate) fn user(&self) -> i64 {
        self.owner.as_ref().map_or(0, |(_, user)| *user)
    }

    pub(crate) fn me(&self) -> String {
        self.device.clone().unwrap_or_default()
    }

    pub(crate) fn label_of(&self, id: &str) -> Option<String> {
        if self.device.as_deref() == Some(id) {
            return Some(self.label.clone());
        }
        let (_, doc) = self.replica.bases.get(&format!("device/{id}"))?;
        doc.view()?.get("name")?.as_str().map(String::from)
    }

    pub(crate) fn active(&self) -> bool {
        self.device.is_some() && (self.ready || self.stale) && !self.paused
    }

    pub(crate) fn state(&self) -> &'static str {
        match () {
            _ if self.url.is_empty() => "none",
            _ if link::shown().is_some() => "linking",
            _ if self.device.is_none() => "unlinked",
            _ if !self.ready => "needs_key",
            _ if self.paused => "paused",
            _ if self.last_error.is_some() => "error",
            _ => "ready",
        }
    }
}

pub(crate) fn failed(message: impl Into<String>) -> Error {
    Error::Account(message.into())
}

pub(crate) fn read<T>(look: impl FnOnce(&Saved) -> T) -> T {
    let mut saved = SAVED.lock().unwrap_or_else(PoisonError::into_inner);
    let file = || std::fs::read(store::dir().ok()?.join(FILE)).ok();
    look(
        saved.get_or_insert_with(|| file().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()),
    )
}

pub(crate) fn update(change: impl FnOnce(&mut Saved)) -> Result<(), Error> {
    let mut saved = read(Saved::clone);
    change(&mut saved);
    store::write(&store::dir()?.join(FILE), &serde_json::to_vec(&saved)?)?;
    *SAVED.lock().unwrap_or_else(PoisonError::into_inner) = Some(saved);
    Ok(())
}

pub(crate) fn linked() -> Result<(), Error> {
    match read(|s| (s.url.is_empty(), s.device.is_some())) {
        (true, _) => Err(failed("add a server first")),
        (false, false) => Err(failed("link this phone to the server first")),
        _ => Ok(()),
    }
}

pub(crate) fn api() -> Result<(Api, Secrets), Error> {
    let secrets = vault::load()?;
    Ok((Api::new(&read(|s| s.url.clone()))?.with_token(&secrets.token), secrets))
}

pub(crate) async fn pause(seconds: u64) -> Result<(), Error> {
    let until = Instant::now() + Duration::from_secs(seconds);
    while Instant::now() < until {
        if CANCEL.load(Relaxed) {
            return Err(failed("stopped"));
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    Ok(())
}

pub(crate) fn active() -> bool {
    read(Saved::active)
}
