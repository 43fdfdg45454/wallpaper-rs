use crate::daemon::Daemon;
use crate::error::{Error, Result, failed, invalid};
use crate::jobs::now;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use wprs_core::dbus::error::NEEDS_SECRET;
use wprs_library::Source;
use wprs_providers::{Cache, Item, Provider};

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

pub fn failure(e: wprs_providers::Error, secret: bool) -> Error {
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
        cache.allow(if kept { u64::MAX } else { budget - used });
        let path = match provider.fetch(&item, cache).await {
            Ok(path) => path,
            Err(e) if rejected(&e) => return Err(e),
            Err(wprs_providers::Error::TooLarge) => {
                (out.skipped, used) = (out.skipped + 1, budget);
                continue;
            }
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

impl Daemon {
    pub fn protected(&self, source: &Source) -> Result<HashSet<PathBuf>> {
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
}
