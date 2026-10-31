use crate::compositor::lock;
use crate::daemon::Daemon;
use crate::error::{Error, Result, failed, invalid, not_found};
use crate::jobs::now;
use crate::keyring::Keyring;
use crate::kinds::{Options, entry};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::UNIX_EPOCH;
use tokio::sync::Notify;
use wprs_core::dbus::error::NEEDS_SECRET;
use wprs_core::model::Credit;
use wprs_library::Source;

pub const CREDITS: &str = ".credits.json";
const MAX_SECRET: usize = 4096;

#[derive(Default)]
pub struct Health {
    pub last_error: Option<String>,
    pub failures: u32,
    pub retry_at: Option<i64>,
    pub rejected: bool,
    pub locked: bool,
}

#[derive(Default)]
pub struct Remote {
    pub keyring: OnceLock<Keyring>,
    pub health: Mutex<HashMap<String, Health>>,
    pub syncing: Mutex<HashSet<String>>,
    pub adding: Mutex<HashSet<String>>,
    pub credits: Mutex<HashMap<PathBuf, Credit>>,
    pub poke: Notify,
    pub repo_errors: Mutex<HashMap<String, String>>,
    pub removing: Mutex<HashSet<String>>,
    pub previews: Mutex<Vec<(String, String)>>,
    pub preview_wake: Notify,
    pub orphans: Mutex<BTreeSet<String>>,
    pub orphan_wake: Notify,
}

pub struct Claim<'a>(&'a Mutex<HashSet<String>>, String);

impl Drop for Claim<'_> {
    fn drop(&mut self) {
        lock(self.0).remove(&self.1);
    }
}

pub fn claim<'a>(set: &'a Mutex<HashSet<String>>, id: &str) -> Option<Claim<'a>> {
    lock(set).insert(id.to_owned()).then(|| Claim(set, id.to_owned()))
}

pub fn start(daemon: &Arc<Daemon>, bus: zbus::Connection) {
    daemon.remote.keyring.set(Keyring::new(bus.clone())).ok();
    daemon.load_orphans();
    tokio::spawn(crate::orphans::watch(daemon.clone(), bus));
    for (id, value) in daemon.state().config.providers.iter() {
        if let Err(e) = entry(value) {
            tracing::warn!(provider = id, "skipping a provider whose configuration cannot be used");
            tracing::debug!(provider = id, "{e}");
        }
    }
    let daemon = daemon.clone();
    tokio::task::spawn_blocking(move || {
        let dirs = fs::read_dir(daemon.store.cache.join("providers")).into_iter().flatten().flatten();
        let dirs = dirs.filter_map(|dir| fs::canonicalize(dir.path()).ok());
        dirs.for_each(|dir| daemon.remember_credits(&dir, &read_credits(&dir)));
        tokio::spawn(crate::sync::schedule(daemon.clone()));
        tokio::spawn(crate::catalogue::auto_refresh(Arc::downgrade(&daemon)));
        tokio::spawn(crate::previews::previews(daemon));
    });
}

pub fn needs_secret(id: &str) -> Error {
    Error::Named(NEEDS_SECRET, format!("{id} needs its secret; store it with SetSecret"))
}

pub fn check_secret(secret: &str) -> Result<()> {
    if secret.len() > MAX_SECRET || secret.chars().any(char::is_control) {
        return Err(invalid("a secret has at most 4096 bytes and no control characters"));
    }
    Ok(())
}

pub fn read_credits(dir: &Path) -> BTreeMap<String, Credit> {
    fs::read(dir.join(CREDITS)).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default()
}

pub fn last_sync(dir: &Path) -> Option<i64> {
    let modified = fs::metadata(dir.join(CREDITS)).and_then(|m| m.modified()).ok()?;
    modified.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs() as i64)
}

impl Daemon {
    pub fn library_ready(&self) -> Result<()> {
        self.store.error.as_ref().map_or(Ok(()), |e| Err(failed(e.clone())))
    }

    pub fn keyring(&self) -> Result<&Keyring> {
        self.remote.keyring.get().ok_or_else(|| failed("the keyring is not connected yet"))
    }

    pub fn provider(&self, id: &str) -> Result<crate::kinds::Entry> {
        let value = self.state().config.providers.get(id).cloned();
        let value = value.ok_or_else(|| not_found(format!("provider {id:?} not found")))?;
        entry(&value).map_err(|e| invalid(format!("the configuration of {id} cannot be used: {e}")))
    }

    pub fn provider_dir(&self, id: &str) -> PathBuf {
        self.store.cache.join("providers").join(id)
    }

    pub fn provider_source(&self, id: &str) -> Option<Source> {
        let sources = self.store.read(|lib| lib.sources()).unwrap_or_default();
        sources.into_iter().find(|s| self.provider_of(&s.path).as_deref() == Some(id))
    }

    pub fn credit(&self, path: &Path) -> Option<Credit> {
        lock(&self.remote.credits).get(path).cloned()
    }

    pub fn remember_credits(&self, dir: &Path, credits: &BTreeMap<String, Credit>) {
        let mut known = lock(&self.remote.credits);
        known.retain(|path, _| !path.starts_with(dir));
        known.extend(credits.iter().map(|(name, credit)| (dir.join(name), credit.clone())));
    }

    pub fn save_credits(&self, dir: &Path, credits: &BTreeMap<String, Credit>) -> Result<()> {
        let mut file = tempfile::Builder::new().prefix(".tmp-").tempfile_in(dir)?;
        file.write_all(&serde_json::to_vec(credits).map_err(|e| failed(e.to_string()))?)?;
        file.persist(dir.join(CREDITS)).map_err(|e| Error::from(e.error))?;
        self.remember_credits(dir, credits);
        Ok(())
    }

    pub fn next_sync(&self, id: &str, options: &Options) -> Option<i64> {
        let refresh = options.get("refresh").and_then(|r| wprs_schedule::parse_interval(r))?.as_secs() as i64;
        let health = lock(&self.remote.health);
        let health = health.get(id);
        if refresh == 0 || health.is_some_and(|h| h.rejected) {
            return None;
        }
        let last = last_sync(&self.provider_dir(id)).filter(|at| *at <= now());
        let due = last.map_or_else(now, |at| at + refresh);
        Some(due.max(health.and_then(|h| h.retry_at).unwrap_or_default()))
    }

    pub fn record(&self, id: &str, result: &Result<Option<wprs_core::model::SyncReport>>) {
        {
            let mut health = lock(&self.remote.health);
            let health = health.entry(id.to_owned()).or_default();
            match result {
                Ok(Some(_)) => *health = Health::default(),
                Ok(None) => {}
                Err(Error::Cancelled) => health.retry_at = Some(now() + 3600),
                Err(e) => {
                    let (name, message) = e.parts();
                    tracing::warn!(provider = id, error = name, "provider sync failed");
                    health.failures += 1;
                    let backoff = (30_i64 << (health.failures - 1).min(7)).min(3600);
                    (health.retry_at, health.rejected) = (Some(now() + backoff), name == NEEDS_SECRET);
                    health.last_error = Some(message);
                }
            }
        }
        self.changes.changed("providers", vec![id.to_owned()]);
        self.remote.poke.notify_one();
    }
}
