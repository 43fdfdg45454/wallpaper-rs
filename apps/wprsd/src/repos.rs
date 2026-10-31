use crate::compositor::lock;
use crate::daemon::Daemon;
use crate::error::{Error, Result, failed, invalid, not_found};
use crate::items::reply;
use crate::jobs::{Ctl, now};
use crate::kinds::{local, parse_url};
use serde_json::Value;
use settle::{Settled, settle, vanished};
use std::sync::Arc;
use wprs_core::dbus::error::{EXISTS, NEWER, STALE};
use wprs_core::model::{JobKind, MODEL, RefreshOutput, Repo, RepoWarning, ReposReply, Update};
use wprs_format::{Anchor, RepoInfo};
use wprs_providers::HttpFetch;

mod settle;

impl From<wprs_format::Error> for Error {
    fn from(e: wprs_format::Error) -> Self {
        use wprs_format::Error as F;
        match e {
            F::NotFound(_) => not_found(e.to_string()),
            F::Exists(_) => Error::Named(EXISTS, e.to_string()),
            F::Invalid { reason, .. } => invalid(reason),
            F::UnsafePath(_) => invalid(e.to_string()),
            F::Stale(_) => Error::Named(STALE, e.to_string()),
            F::Newer(_) => Error::Named(NEWER, e.to_string()),
            F::Io(io) if io.kind() == std::io::ErrorKind::Interrupted => Error::Cancelled,
            e => failed(e.to_string()),
        }
    }
}

pub fn newer(a: &str, b: &str) -> bool {
    semver::Version::parse(a).ok() > semver::Version::parse(b).ok()
}

pub fn fetcher(ctl: &Ctl, bases: &[String]) -> Result<HttpFetch> {
    Ok(HttpFetch::new(ctl.flag()).map_err(|e| failed(e.to_string()))?.within(bases))
}

fn warning(text: &str) -> RepoWarning {
    match (text.starts_with("root"), text.contains("within")) {
        (true, true) => RepoWarning::RootExpiring,
        (true, false) => RepoWarning::RootExpired,
        (false, true) => RepoWarning::EntryExpiring,
        (false, false) => RepoWarning::Stale,
    }
}

impl Daemon {
    fn repo_model(&self, info: RepoInfo) -> Repo {
        let host =
            |base: &String| reqwest::Url::parse(base).ok().and_then(|u| u.host_str().map(String::from));
        let hosts = info.hosts.iter().map(|base| host(base).unwrap_or_else(|| base.clone())).collect();
        let last_error = lock(&self.remote.repo_errors).get(&info.repo_id).cloned();
        let added = info.url.split('?').next().unwrap_or_default();
        let jobs = [
            (JobKind::RepoRefresh, info.repo_id.as_str()),
            (JobKind::RepoRefresh, ""),
            (JobKind::RepoAdd, added),
        ];
        let job = jobs.iter().flat_map(|(kind, target)| self.jobs.find(*kind, target)).min();
        Repo {
            hosts,
            name: info.name,
            url: info.url,
            fingerprints: info.fingerprints,
            root_version: info.root_version,
            root_expires: info.root_expires.timestamp(),
            entry_version: info.entry_version,
            entry_expires: info.entry_expires.timestamp(),
            checked: info.checked.timestamp(),
            stale: info.stale,
            packages: info.packages as u32,
            warnings: info.warnings.iter().map(|w| warning(w)).collect(),
            last_error,
            job,
            broken: info.broken,
            repo_id: info.repo_id,
        }
    }

    pub fn list_repos(&self) -> Result<String> {
        let serial = self.changes.serial();
        let repos =
            wprs_format::repos(&self.store.data, now())?.into_iter().map(|i| self.repo_model(i)).collect();
        reply(ReposReply { model: MODEL, serial, repos })
    }

    pub fn bases(&self, repo_id: &str) -> Vec<String> {
        let repos = wprs_format::repos(&self.store.data, now()).unwrap_or_default();
        repos.into_iter().find(|r| r.repo_id == repo_id).map(|r| r.hosts).unwrap_or_default()
    }

    pub fn repo_id(&self, repo: &str) -> Result<String> {
        let repo = repo.trim().to_ascii_lowercase();
        if !(8..=64).contains(&repo.len()) || !repo.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(invalid("give a repository id or at least its first 8 hex digits"));
        }
        let ids = wprs_format::repos(&self.store.data, now())?.into_iter().map(|r| r.repo_id);
        match ids.filter(|id| id.starts_with(&repo)).collect::<Vec<_>>().as_slice() {
            [one] => Ok(one.clone()),
            [] => Err(not_found(format!("repository {repo} not found"))),
            _ => Err(invalid(format!("{repo} matches several repositories; give more digits"))),
        }
    }

    fn gone(&self, id: &str) -> bool {
        vanished(&lock(&self.remote.removing), &self.store.data, id)
    }

    pub fn add_repo(self: &Arc<Self>, url: &str) -> Result<u64> {
        let parsed = parse_url(url, true)?;
        if !(matches!(parsed.scheme(), "https" | "file") || parsed.scheme() == "http" && local(&parsed)) {
            return Err(invalid("use an https:// or file:// URL, or http:// to this computer"));
        }
        let anchor = Anchor::parse(url)?;
        let known = wprs_format::repos(&self.store.data, now())?;
        let sound = |id: &String| known.iter().any(|r| &r.repo_id == id && r.broken.is_none());
        if let Some(id) = anchor.repo_id.filter(sound) {
            return Err(Error::Named(EXISTS, format!("repository {id} is already added")));
        }
        let (url, target) = (url.to_owned(), url.split('?').next().unwrap_or_default());
        Ok(self.start_job(JobKind::RepoAdd, target, move |d, ctl| {
            let info = wprs_format::add_repo(&d.store.data, &fetcher(ctl, &[])?, &url, now())?;
            lock(&d.remote.repo_errors).remove(&info.repo_id);
            tracing::info!(repo = &info.repo_id[..8], "repository added");
            d.changes.changed("repos", vec![info.repo_id.clone()]);
            Ok(serde_json::to_value(d.repo_model(info)).unwrap_or_default())
        }))
    }

    pub fn refresh_repos(self: &Arc<Self>, repo: &str) -> Result<u64> {
        let target = if repo.is_empty() { String::new() } else { self.repo_id(repo)? };
        Ok(self.refresh_job(&target))
    }

    pub fn refresh_job(self: &Arc<Self>, target: &str) -> u64 {
        let owned = target.to_owned();
        self.start_job(JobKind::RepoRefresh, target, move |d, ctl| d.refresh(&owned, ctl))
    }

    fn refresh(self: &Arc<Self>, target: &str, ctl: &Ctl) -> Result<Value> {
        let known = wprs_format::repos(&self.store.data, now())?;
        let ids: Vec<String> = match target {
            "" => known.iter().map(|r| r.repo_id.clone()).collect(),
            id => vec![id.to_owned()],
        };
        let mut out = RefreshOutput { repos: Vec::new(), updates: Vec::new(), revoked: Vec::new() };
        let mut failure = None;
        for id in ids {
            let bases = known.iter().find(|r| r.repo_id == id).map_or(&[][..], |r| &r.hosts);
            let updated = ctl.check().and_then(|()| {
                Ok(wprs_format::update_repo(&self.store.data, &fetcher(ctl, bases)?, &id, now())?)
            });
            match settle(updated, || self.gone(&id)) {
                Settled::Done(outcome) => {
                    lock(&self.remote.repo_errors).remove(&id);
                    let latest = self.latest().unwrap_or_default();
                    let update = |u: wprs_format::Update| Update {
                        changes: latest
                            .get(&(id.clone(), u.id.clone()))
                            .map(|l| l.0.clone())
                            .unwrap_or_default(),
                        repo_id: id.clone(),
                        id: u.id,
                        installed: u.installed,
                        available: u.available,
                    };
                    out.updates.extend(outcome.updates.into_iter().map(update));
                    out.revoked.extend(outcome.revoked.iter().map(|i| self.package_model(i, None)));
                    out.repos.push(self.repo_model(outcome.repo));
                }
                Settled::Stopped => {
                    failure = Some(Error::Cancelled);
                    break;
                }
                Settled::Gone => {}
                Settled::Failed(e) => {
                    tracing::warn!(repo = &id[..8], "repository refresh failed");
                    lock(&self.remote.repo_errors).insert(id.clone(), e.to_string());
                    failure = failure.or(Some(e)).filter(|_| !target.is_empty());
                }
            }
        }
        self.reload_installed();
        failure.map_or_else(|| Ok(serde_json::to_value(out).unwrap_or_default()), Err)
    }
}
