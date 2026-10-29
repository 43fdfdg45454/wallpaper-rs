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

static NEXT: AtomicU64 = AtomicU64::new(0);

pub const PREVIEW: &str = "preview-";

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
        let path = daemon.make_preview(&spec).await.map(text).unwrap_or_default();
        daemon.changes.send(Signal::Preview(spec, path));
    }
}

impl Daemon {
    fn offer_of(&self, spec: &str) -> Result<(String, String, String, Option<String>)> {
        let (repo, id, version) = parse_spec(spec)?;
        let repo = repo.map(|r| self.repo_id(r)).transpose()?;
        let offers = wprs_format::browse(&self.store.data, id)?.into_iter();
        let offers: Vec<_> =
            offers.filter(|o| o.id == id && repo.as_ref().is_none_or(|r| *r == o.repo_id)).collect();
        let [offer] = offers.as_slice() else {
            return Err(not_found(format!("{spec} matches no single package")));
        };
        let version = version.map_or_else(|| offer.release.version.clone(), String::from);
        let preview = offer.release.preview.as_ref().filter(|_| version == offer.release.version);
        Ok((offer.repo_id.clone(), id.to_owned(), version, preview.map(|p| p.sha256.clone())))
    }

    fn preview_file(&self, sha256: &str) -> PathBuf {
        self.store.cache.join("previews").join(format!("{sha256}.jpg"))
    }

    pub fn request_previews(&self, specs: Vec<String>, sender: &str) -> Result<HashMap<String, String>> {
        limited(&specs, 500)?;
        specs.iter().try_for_each(|spec| parse_spec(spec).map(drop))?;
        let mut cached = HashMap::new();
        let mut queued = Vec::new();
        for spec in specs {
            let known =
                self.offer_of(&spec).ok().and_then(|(.., sha)| sha).map(|sha| self.preview_file(&sha));
            let touched =
                known.filter(|p| File::open(p).and_then(|f| f.set_modified(SystemTime::now())).is_ok());
            match touched {
                Some(path) => drop(cached.insert(spec, text(path))),
                None => queued.push((sender.to_owned(), spec)),
            }
        }
        let mut queue = lock(&self.remote.previews);
        queue.retain(|(from, _)| from != sender);
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
        let (repo_id, id, version, _) = self.offer_of(spec).ok()?;
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
