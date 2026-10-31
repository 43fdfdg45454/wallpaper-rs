use crate::compositor::lock;
use crate::daemon::Daemon;
use crate::error::{Result, invalid};
use crate::items::reply;
use crate::jobs::now;
use crate::repos::newer;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::{Arc, Weak};
use std::time::Duration;
use wprs_core::locale::languages;
use wprs_core::model::{BrowsePage, MODEL, RepoPackage};
use wprs_format::Rated;

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
        let previews = self.previews_of(&id);
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
                repo_name: item.repo_name,
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

    fn previews_of(&self, id: &str) -> Vec<PathBuf> {
        let data = &self.store.data;
        let repos = wprs_format::repos(data, now()).unwrap_or_default().into_iter().map(|r| r.repo_id);
        let shas = |repo: &str| wprs_format::previews(data, repo).unwrap_or_default();
        let others: BTreeSet<String> = repos.filter(|r| r != id).flat_map(|r| shas(&r)).collect();
        let ours = shas(id).into_iter().filter(|sha| !others.contains(sha));
        ours.map(|sha| self.store.cache.join("previews").join(format!("{sha}.jpg"))).collect()
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
