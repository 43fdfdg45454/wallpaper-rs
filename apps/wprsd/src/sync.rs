use crate::compositor::lock;
use crate::daemon::Daemon;
use crate::error::{Error, Result, failed, invalid};
use crate::jobs::{Ctl, now};
use crate::keyring::Stored;
use crate::kinds::{build, entry};
use crate::remote::{claim, needs_secret, read_credits};
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::runtime::Handle;
use wprs_core::dbus::error::NEEDS_SECRET;
use wprs_core::model::{Credit, JobKind, SourceScan, SyncOutput, SyncReport};
use wprs_library::{ScanReport, Source};
use wprs_providers::{Cache, Item, Provider};

const PUBLIC: [&str; 5] = ["bing", "apod", "wikimedia", "wallhaven", "motionbgs"];

#[derive(Default)]
pub struct Fetched {
    pub listed: u32,
    pub downloaded: u32,
    pub reused: u32,
    pub skipped: u32,
    pub failed: u32,
    pub files: Vec<(PathBuf, Item)>,
}

fn rejected(e: &wprs_providers::Error) -> bool {
    matches!(e, wprs_providers::Error::Http(e) if matches!(e.status().map(|s| s.as_u16()), Some(401 | 403)))
}

fn failure(e: wprs_providers::Error, secret: bool) -> Error {
    match e {
        e if secret && rejected(&e) => {
            Error::Named(NEEDS_SECRET, "the server rejected the secret; store it again with SetSecret".into())
        }
        wprs_providers::Error::Invalid(message) if message.contains("plain http") => invalid(message),
        e => failed(e.to_string()),
    }
}

fn modified(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

pub async fn download(
    provider: &dyn Provider,
    cache: &Cache,
    protected: &HashSet<PathBuf>,
    budget: u64,
    live: impl Fn(u64, u64) -> bool,
) -> wprs_providers::Result<Option<Fetched>> {
    let items = provider.list().await?;
    let total = items.len() as u64;
    let (mut out, mut used) = (Fetched { listed: total as u32, ..Fetched::default() }, 0);
    for (done, item) in (0..).zip(items) {
        if !live(done, total) {
            return Ok(None);
        }
        let (kept, before) = (protected.contains(&cache.path(&item)), modified(&cache.path(&item)));
        if used >= budget && !kept {
            out.skipped += 1;
            continue;
        }
        let path = match provider.fetch(&item, cache).await {
            Ok(path) => path,
            Err(e) if rejected(&e) => return Err(e),
            Err(_) => {
                out.failed += 1;
                continue;
            }
        };
        let (size, reused) =
            (fs::metadata(&path).map_or(0, |m| m.len()), before.is_some() && before == modified(&path));
        if !kept && !reused && used + size > budget {
            let mut sidecar = path.clone().into_os_string();
            sidecar.push(".json");
            [&path, Path::new(&sidecar)].iter().for_each(|p| drop(fs::remove_file(p)));
            (out.skipped, used) = (out.skipped + 1, budget);
            continue;
        }
        used += if kept { 0 } else { size };
        (out.reused, out.downloaded) =
            if reused { (out.reused + 1, out.downloaded) } else { (out.reused, out.downloaded + 1) };
        out.files.push((path, item));
    }
    live(total, total);
    Ok(Some(out))
}

pub async fn schedule(daemon: Arc<Daemon>) {
    loop {
        let poked = daemon.remote.poke.notified();
        let wait = daemon.due_syncs().await;
        tokio::select! {
            () = tokio::time::sleep(wait) => {}
            () = poked => tokio::time::sleep(Duration::from_secs(5)).await,
        }
    }
}

impl Daemon {
    async fn due_syncs(self: &Arc<Self>) -> Duration {
        let mut wait = 3600;
        let entries = self.state().config.providers.clone();
        for (id, value) in entries.into_iter().filter(|_| self.store.error.is_none()) {
            let Ok((known, _, options)) = entry(&value) else { continue };
            let Some(left) = self.next_sync(&id, &options).map(|at| at - now()) else { continue };
            if left > 0 {
                wait = wait.min(left);
                continue;
            }
            let busy = [id.as_str(), ""].iter().any(|t| !self.jobs.find(JobKind::ProviderSync, t).is_empty());
            let required = known.secret.as_ref().is_some_and(|s| s.required);
            if !busy && (!required || self.has_secret(&id, Some(&known.kind)).await.unwrap_or(false)) {
                self.sync_job(&id, false);
            }
        }
        Duration::from_secs(wait.max(1) as u64)
    }

    pub fn sync_job(self: &Arc<Self>, target: &str, explicit: bool) -> u64 {
        let owned = target.to_owned();
        let job = self
            .start_job(JobKind::ProviderSync, target, move |d, ctl| d.sync_provider(&owned, explicit, ctl));
        self.changes.changed("providers", Vec::from_iter((!target.is_empty()).then(|| target.to_owned())));
        job
    }

    fn sync_provider(self: &Arc<Self>, target: &str, explicit: bool, ctl: &Ctl) -> Result<Value> {
        let ids: Vec<String> = match target {
            "" => {
                self.state().config.providers.keys().filter(|id| self.provider(id).is_ok()).cloned().collect()
            }
            id => vec![id.to_owned()],
        };
        let mut reports = Vec::new();
        for id in ids {
            let Some(_claim) = claim(&self.remote.syncing, &id) else { continue };
            let result = ctl.check().and_then(|()| self.sync_one(&id, explicit, target.is_empty(), ctl));
            self.record(&id, &result);
            match result {
                Ok(report) => reports.extend(report),
                Err(e) if target.is_empty() && !matches!(e, Error::Cancelled) => {}
                Err(e) => return Err(e),
            }
        }
        Ok(serde_json::to_value(SyncOutput { reports }).unwrap_or_default())
    }

    fn sync_one(
        self: &Arc<Self>,
        id: &str,
        explicit: bool,
        all: bool,
        ctl: &Ctl,
    ) -> Result<Option<SyncReport>> {
        let (known, _, options) = self.provider(id)?;
        let kind = known.kind.as_str();
        let runtime = Handle::current();
        let secret = match &known.secret {
            None => None,
            Some(field) => match runtime.block_on(self.keyring()?.read(id, explicit))? {
                Stored::Secret(secret) => Some(secret),
                Stored::Locked => {
                    let refresh =
                        wprs_schedule::parse_interval(&options["refresh"]).map_or(0, |d| d.as_secs());
                    let mut health = lock(&self.remote.health);
                    let health = health.entry(id.to_owned()).or_default();
                    (health.locked, health.retry_at) = (true, Some(now() + refresh.max(900) as i64));
                    return Ok(None);
                }
                Stored::Missing if field.required && all => return Ok(None),
                Stored::Missing if field.required => return Err(needs_secret(id)),
                Stored::Missing => None,
            },
        };
        let provider =
            build(kind, &options, secret.as_deref().map(String::as_str)).map_err(|e| failure(e, false))?;
        let source = self.provider_source(id);
        let source = source.ok_or_else(|| {
            failed(format!("the library source of {id} is missing; add the provider again"))
        })?;
        let limit = options["cache_mb"].parse::<u64>().unwrap_or(1024) << 20;
        let cache = Cache::new(&source.path, limit).map_err(|e| failed(e.to_string()))?;
        let protected = self.protected(&source)?;
        let held: u64 = protected.iter().filter_map(|p| fs::metadata(p).ok()).map(|m| m.len()).sum();
        let live = |done, total| {
            ctl.progress(done, total);
            ctl.check().is_ok()
        };
        let fetched = runtime.block_on(download(
            provider.as_ref(),
            &cache,
            &protected,
            limit.saturating_sub(held),
            live,
        ));
        drop((provider, secret));
        let fetched = fetched.map_err(|e| failure(e, known.secret.is_some()))?.ok_or(Error::Cancelled)?;
        cache.evict_except(&self.protected(&source)?).map_err(|e| failed(e.to_string()))?;
        let _turn = self.jobs.wait_turn(JobKind::Scan);
        let scan = self.store_items(id, kind, &source, &fetched)?;
        let Fetched { listed, downloaded, reused, skipped, failed, .. } = fetched;
        Ok(Some(SyncReport { provider: id.into(), listed, downloaded, reused, skipped, failed, scan }))
    }

    fn protected(&self, source: &Source) -> Result<HashSet<PathBuf>> {
        let mut paths: HashSet<PathBuf> = {
            let st = self.state();
            let sections = std::iter::once(&st.config.default).chain(st.config.outputs.values());
            sections.filter_map(|s| fs::canonicalize(s.image.as_ref()?).ok()).collect()
        };
        let query = |filter: &str| format!("source:{} {filter}", source.uid);
        self.store.read(|lib| {
            for filter in ["fav:yes", "rating:>0"] {
                paths.extend(lib.search(&query(filter), now(), 0, u32::MAX)?.into_iter().map(|i| i.path));
            }
            for playlist in lib.playlists()?.into_iter().filter(|p| p.query.is_none()) {
                let items = lib.playlist_items(&playlist.uid, now())?.into_iter();
                paths.extend(items.filter(|i| i.source.as_deref() == Some(&source.uid)).map(|i| i.path));
            }
            let shown = lib.rotations()?.into_iter().filter_map(|r| lib.item(r.current.as_deref()?).ok());
            paths.extend(shown.map(|i| i.path));
            Ok::<_, wprs_library::Error>(())
        })?;
        paths.retain(|p| p.starts_with(&source.path));
        Ok(paths)
    }

    fn store_items(
        self: &Arc<Self>,
        id: &str,
        kind: &str,
        source: &Source,
        fetched: &Fetched,
    ) -> Result<SourceScan> {
        let mut lib = self.store.connect()?;
        let report = lib.scan(&source.uid, now())?;
        let mut credits = read_credits(&source.path);
        credits.retain(|name, _| source.path.join(name).exists());
        for (path, item) in &fetched.files {
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let link = PUBLIC.contains(&kind).then(|| item.url.clone());
            let (attribution, license) = (item.attribution.clone(), item.license.clone());
            credits.insert(
                name,
                Credit { provider: id.into(), title: item.title.clone(), attribution, license, link },
            );
            let title: String = item.title.chars().filter(|c| !c.is_control()).take(256).collect();
            let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned());
            let found =
                path.to_str().and_then(|p| lib.resolve_item(p).ok()).and_then(|uid| lib.item(&uid).ok());
            if let Some(found) = found.filter(|i| Some(&i.title) == stem.as_ref() && !title.trim().is_empty())
            {
                lib.set_title(&found.uid, title.trim()).ok();
            }
        }
        self.save_credits(&source.path, &credits)?;
        self.library_changed(Vec::new());
        self.changes.changed("state", Vec::new());
        if report.added + report.updated + report.moved > 0 {
            self.analyze_job();
        }
        let ScanReport { added, updated, moved, missing } = report;
        Ok(SourceScan { source: Some(source.uid.clone()), added, updated, moved, missing })
    }
}

#[cfg(test)]
mod tests;
