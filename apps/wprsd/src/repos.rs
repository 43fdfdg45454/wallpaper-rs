use crate::compositor::lock;
use crate::daemon::Daemon;
use crate::error::{Error, Result, failed, invalid, not_found};
use crate::items::reply;
use crate::jobs::{Ctl, now};
use crate::kinds::{local, parse_url};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Weak};
use std::time::Duration;
use wprs_core::dbus::error::{EXISTS, NEWER, STALE};
use wprs_core::locale::languages;
use wprs_core::model::{
    BrowsePage, JobKind, MODEL, RefreshOutput, Repo, RepoPackage, RepoWarning, ReposReply, Update,
};
use wprs_format::{Anchor, Rated, RepoInfo};
use wprs_providers::HttpFetch;

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

pub async fn auto_refresh(daemon: Weak<Daemon>) {
    let mut due = now() + 600;
    loop {
        due = due.min(now() + 86_400);
        tokio::time::sleep(Duration::from_secs((due - now()).clamp(1, 3600) as u64)).await;
        if now() < due {
            continue;
        }
        due = now() + 86_400;
        let Some(daemon) = daemon.upgrade() else { return };
        tokio::task::spawn_blocking(move || {
            let some = wprs_format::repos(&daemon.store.data, now()).is_ok_and(|repos| !repos.is_empty());
            if some && daemon.state().config.repos.auto_refresh {
                daemon.refresh_job("");
            }
        });
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

    pub fn add_repo(self: &Arc<Self>, url: &str) -> Result<u64> {
        let parsed = parse_url(url, true)?;
        if !(matches!(parsed.scheme(), "https" | "file") || parsed.scheme() == "http" && local(&parsed)) {
            return Err(invalid("use an https:// or file:// URL, or http:// to this computer"));
        }
        let anchor = Anchor::parse(url)?;
        let known = wprs_format::repos(&self.store.data, now())?;
        if let Some(id) = anchor.repo_id.filter(|id| known.iter().any(|r| &r.repo_id == id)) {
            return Err(Error::Named(EXISTS, format!("repository {id} is already added")));
        }
        let (url, target) = (url.to_owned(), url.split('?').next().unwrap_or_default());
        Ok(self.start_job(JobKind::RepoAdd, target, move |d, ctl| {
            let info = wprs_format::add_repo(&d.store.data, &fetcher(ctl, &[])?, &url, now())?;
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
            match updated {
                Ok(outcome) => {
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
                Err(Error::Cancelled) => {
                    failure = Some(Error::Cancelled);
                    break;
                }
                Err(e) => {
                    tracing::warn!(repo = &id[..8], "repository refresh failed");
                    lock(&self.remote.repo_errors).insert(id.clone(), e.to_string());
                    failure = failure.or(Some(e)).filter(|_| !target.is_empty());
                }
            }
        }
        self.reload_installed();
        failure.map_or_else(|| Ok(serde_json::to_value(out).unwrap_or_default()), Err)
    }

    pub fn remove_repo(self: &Arc<Self>, repo: &str, uninstall: bool) -> Result<()> {
        let id = self.repo_id(repo)?;
        let packages: Vec<_> =
            wprs_format::installed(&self.store.data)?.into_iter().filter(|i| i.repo_id == id).collect();
        if uninstall {
            self.library_ready()?;
            packages.iter().try_for_each(|p| self.shown_on(&p.path))?;
        }
        let ours =
            |job: &wprs_core::model::Job| job.target == id || job.target.starts_with(&format!("{id}/"));
        let _removing = crate::remote::claim(&self.remote.removing, &id);
        loop {
            let jobs = self.jobs.list().into_iter().filter(|j| j.finished_at.is_none() && ours(j));
            let jobs: Vec<u64> = jobs.map(|job| job.id).collect();
            if jobs.is_empty() {
                break;
            }
            jobs.iter().try_for_each(|job| self.cancel_job(*job))?;
            self.jobs.wait(&jobs);
        }
        let previews: Vec<PathBuf> = wprs_format::browse(&self.store.data, "")?
            .into_iter()
            .filter(|item| item.repo_id == id)
            .filter_map(|item| {
                Some(self.store.cache.join("previews").join(format!("{}.jpg", item.release.preview?.sha256)))
            })
            .collect();
        if uninstall {
            let ids: std::collections::BTreeSet<String> = packages.into_iter().map(|p| p.id).collect();
            ids.iter().try_for_each(|package| self.uninstall(package, ""))?;
        }
        wprs_format::remove_repo(&self.store.data, &id)?;
        previews.iter().for_each(|path| drop(std::fs::remove_file(path)));
        lock(&self.remote.repo_errors).remove(&id);
        tracing::info!(repo = &id[..8], "repository removed");
        self.reload_installed();
        Ok(())
    }

    pub fn browse(&self, query: &str, offset: u32, limit: u32) -> Result<String> {
        if limit > 500 {
            return Err(invalid("limit must be at most 500"));
        }
        let serial = self.changes.serial();
        let names: HashMap<String, String> =
            wprs_format::repos(&self.store.data, now())?.into_iter().map(|r| (r.repo_id, r.name)).collect();
        let hide = self.state().config.library.hide_mature;
        let mature = |i: &wprs_format::Item| i.release.content.as_ref().is_some_and(Rated::mature);
        let items: Vec<_> = wprs_format::browse(&self.store.data, query)?
            .into_iter()
            .filter(|i| !hide || !mature(i))
            .collect();
        let total = items.len() as u32;
        let langs = languages();
        let package = |item: wprs_format::Item| {
            let release = item.release;
            let (title, description) = (release.title_in(&langs), release.description_in(&langs));
            let changes = release.changes.as_ref().map(|c| c.pick(&langs)).unwrap_or_default();
            let update =
                !item.installed.is_empty() && item.installed.iter().all(|v| newer(&release.version, v));
            RepoPackage {
                repo_name: names.get(&item.repo_id).cloned().unwrap_or_default(),
                repo_id: item.repo_id,
                id: item.id,
                author: item.author,
                version: release.version,
                kind: release.kind.to_string(),
                title,
                description,
                tags: release.tags,
                license: release.license,
                authors: release.authors,
                size: release.size,
                published: release.published.timestamp(),
                preview: release.preview.is_some(),
                versions: item.versions,
                installed: item.installed,
                update,
                content: release.content.map(|c| if c.mature() { "mature" } else { "general" }.into()),
                photosensitive: release.photosensitive,
                facets: release.facets,
                changes,
            }
        };
        let packages = items.into_iter().skip(offset as usize).take(limit as usize).map(package).collect();
        reply(BrowsePage { model: MODEL, serial, query: query.into(), total, offset, packages })
    }

    pub fn reload_installed(&self) {
        let installed = wprs_format::installed(&self.store.data).unwrap_or_default();
        let revoked: Vec<PathBuf> =
            installed.iter().filter(|i| i.revoked.is_some()).map(|i| i.path.clone()).collect();
        let shown: Vec<String> = {
            let mut st = self.state();
            st.installed = installed;
            let showing = |o: &&String| st.config.resolve(o).image.is_some_and(|p| revoked.contains(&p));
            st.outputs.keys().filter(showing).cloned().collect()
        };
        shown.iter().for_each(|output| drop(self.configure_output(output)));
        self.rotations.wake.notify_one();
        self.changes.changed("repos", Vec::new());
    }
}
