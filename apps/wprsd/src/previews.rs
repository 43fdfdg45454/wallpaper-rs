use crate::changes::Signal;
use crate::compositor::lock;
use crate::daemon::Daemon;
use crate::error::{Result, not_found};
use crate::items::limited;
use crate::packages::parse_spec;
use crate::store::private_dir;
use crate::thumbs::Req;
use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};
use wprs_library::{Item, Kind, Meta};
use wprs_providers::HttpFetch;

mod moving;

static NEXT: AtomicU64 = AtomicU64::new(0);

pub const PREVIEW: &str = "preview-";
pub const ANIMATED: &str = "animated:";

type Offer = (String, String, String, Option<wprs_format::Preview>);

fn text(path: PathBuf) -> String {
    path.to_string_lossy().into_owned()
}

pub async fn previews(daemon: Arc<Daemon>) {
    loop {
        let wake = daemon.remote.preview_wake.notified();
        let next = {
            let mut queue = lock(&daemon.remote.previews);
            (!queue.is_empty()).then(|| queue.remove(0))
        };
        let Some((_, spec)) = next else {
            wake.await;
            continue;
        };
        let path = match spec.strip_prefix(ANIMATED) {
            Some(moving) => {
                let (daemon, moving) = (daemon.clone(), moving.to_owned());
                tokio::task::spawn_blocking(move || daemon.fetch_animated(&moving)).await.ok().flatten()
            }
            None => daemon.make_preview(&spec).await,
        };
        let path = path.map(text).unwrap_or_default();
        daemon.changes.send(Signal::Preview(spec, path));
    }
}

impl Daemon {
    fn offer_of(&self, spec: &str, animated: bool) -> Result<Offer> {
        let (repo, id, version) = parse_spec(spec)?;
        let repo = repo.map(|r| self.repo_id(r)).transpose()?;
        let offers = wprs_format::browse(&self.store.data, id)?.into_iter();
        let offers: Vec<_> =
            offers.filter(|o| o.id == id && repo.as_ref().is_none_or(|r| *r == o.repo_id)).collect();
        let [offer] = offers.as_slice() else {
            return Err(not_found(format!("{spec} matches no single package")));
        };
        let version = version.map_or_else(|| offer.release.version.clone(), String::from);
        let release = match version == offer.release.version {
            true => offer.release.clone(),
            false => wprs_format::release(&self.store.data, &offer.repo_id, id, &version)?,
        };
        let preview = if animated { release.animated } else { release.preview };
        Ok((offer.repo_id.clone(), id.to_owned(), version, preview))
    }

    fn preview_file(&self, sha256: &str) -> PathBuf {
        self.store.cache.join("previews").join(format!("{sha256}.jpg"))
    }

    fn cached(&self, spec: &str, animated: bool) -> Option<PathBuf> {
        let (.., found) = self.offer_of(spec, animated).ok()?;
        let found = found?;
        let path = if animated {
            self.moving_file(&found.sha256, &found.ext)
        } else {
            self.preview_file(&found.sha256)
        };
        File::open(&path).and_then(|f| f.set_modified(SystemTime::now())).is_ok().then_some(path)
    }

    pub fn request_previews(&self, specs: Vec<String>, sender: &str) -> Result<HashMap<String, String>> {
        self.queue_previews(specs, sender, false)
    }

    pub fn request_animated(&self, specs: Vec<String>, sender: &str) -> Result<HashMap<String, String>> {
        self.queue_previews(specs, sender, true)
    }

    fn queue_previews(
        &self,
        specs: Vec<String>,
        sender: &str,
        animated: bool,
    ) -> Result<HashMap<String, String>> {
        limited(&specs, 500)?;
        specs.iter().try_for_each(|spec| parse_spec(spec).map(drop))?;
        let mut cached = HashMap::new();
        let mut queued = Vec::new();
        for spec in specs {
            match self.cached(&spec, animated) {
                Some(path) => drop(cached.insert(spec, text(path))),
                None if animated => queued.push((sender.to_owned(), format!("{ANIMATED}{spec}"))),
                None => queued.push((sender.to_owned(), spec)),
            }
        }
        let mut queue = lock(&self.remote.previews);
        queue.retain(|(from, spec)| from != sender || spec.starts_with(ANIMATED) != animated);
        queue.extend(queued);
        self.remote.preview_wake.notify_one();
        Ok(cached)
    }

    async fn make_preview(self: &Arc<Self>, spec: &str) -> Option<PathBuf> {
        let (daemon, spec) = (self.clone(), spec.to_owned());
        let fetched = tokio::task::spawn_blocking(move || daemon.fetch_preview(&spec)).await.ok()??;
        let (download, dest) = fetched;
        if download == dest {
            return Some(dest);
        }
        let item = Item {
            uid: format!("{PREVIEW}{}", NEXT.fetch_add(1, Ordering::Relaxed)),
            source: None,
            kind: Kind::Image,
            path: download.clone(),
            title: String::new(),
            size: 0,
            mtime: 0,
            meta: Meta { error: Some("preview".into()), ..Meta::default() },
            package: None,
            tags: Vec::new(),
            rating: 0,
            favorite: false,
            added_at: 0,
            used_at: None,
            use_count: 0,
            missing: false,
        };
        std::fs::remove_file(self.thumbs.path(&item, 1024)).ok();
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.thumbs.queue(item, Req::Wait(1024, tx));
        let made = tokio::time::timeout(Duration::from_secs(30), rx).await;
        let moved =
            made.ok().and_then(Result::ok).and_then(Result::ok).map(|thumb| std::fs::rename(thumb, &dest));
        std::fs::remove_file(&download).ok();
        moved.and_then(Result::ok).map(|()| dest)
    }

    fn fetch_preview(&self, spec: &str) -> Option<(PathBuf, PathBuf)> {
        let (repo_id, id, version, known) = self.offer_of(spec, false).ok()?;
        let known = known.map(|p| p.sha256);
        if let Some(cached) = known.map(|sha| self.preview_file(&sha)).filter(|p| p.is_file()) {
            return Some((cached.clone(), cached));
        }
        let dir = self.store.cache.join("previews");
        private_dir(&dir).ok()?;
        let download = dir.join(format!(".tmp-{}", NEXT.fetch_add(1, Ordering::Relaxed)));
        let fetch = HttpFetch::new(Arc::default()).ok()?.within(&self.bases(&repo_id));
        let found = wprs_format::preview(&self.store.data, &fetch, &repo_id, &id, &version, &download);
        if !found.inspect_err(|_| tracing::debug!("cannot download a preview")).ok()? {
            return None;
        }
        let sha256 = wprs_format::sha256_file(&download).ok()?;
        let dest = self.preview_file(&sha256);
        if dest.is_file() {
            std::fs::remove_file(&download).ok();
            return Some((dest.clone(), dest));
        }
        Some((download, dest))
    }
}
