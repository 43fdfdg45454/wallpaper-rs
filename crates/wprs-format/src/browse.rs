use crate::broken::{scan, sound};
use crate::install::records;
use crate::store::{Snapshot, locked};
use crate::{Listing, Release, Result, Text};
use semver::Version;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Item {
    pub repo_id: String,
    pub repo_name: String,
    pub id: String,
    pub author: Option<String>,
    pub release: Release,
    pub versions: Vec<String>,
    pub installed: Vec<String>,
    pub featured: bool,
}

pub fn browse(data: &Path, query: &str) -> Result<Vec<Item>> {
    browse_with(data, query, false)
}

pub fn browse_with(data: &Path, query: &str, unlisted: bool) -> Result<Vec<Item>> {
    let query = query.to_lowercase();
    let (snaps, installed) = locked(data, false, || Ok((sound(scan(data)?), records(data)?)))?;
    let mut items = Vec::new();
    for snap in snaps {
        let picked = picked(&snap);
        for listing in snap.shards.iter().flat_map(|shard| &shard.packages) {
            let here = installed.iter().filter(|i| i.repo_id == snap.state.repo_id && i.id == listing.id);
            let mut open = available(&snap, listing);
            if here.clone().next().is_none() && !unlisted {
                open.retain(|r| r.shown());
            }
            let Some(release) = open.last().filter(|r| matches(&query, &listing.id, r)) else { continue };
            if release.hidden && here.clone().next().is_none() {
                continue;
            }
            items.push(Item {
                repo_id: snap.state.repo_id.clone(),
                repo_name: snap.root.value.name.clone(),
                id: listing.id.clone(),
                author: listing.author.clone(),
                release: (*release).clone(),
                versions: open.iter().map(|r| r.version.clone()).collect(),
                installed: here.map(|i| i.version.clone()).collect(),
                featured: picked.contains(&listing.id),
            });
        }
    }
    items.sort_by(|a, b| (&a.id, &a.repo_id).cmp(&(&b.id, &b.repo_id)));
    Ok(items)
}

fn picked(snap: &Snapshot) -> Vec<String> {
    let blob = snap.details.as_ref().and_then(|d| d.value.featured.as_ref());
    let bytes = blob.and_then(|b| snap.blobs.get(&b.sha256));
    let found = bytes.and_then(|b| crate::Featured::parse(b, &snap.state.repo_id).ok());
    found.map(|f| f.items.into_iter().map(|p| p.id).collect()).unwrap_or_default()
}

pub(crate) fn listing<'a>(snap: &'a Snapshot, id: &str) -> Option<&'a Listing> {
    snap.shards.iter().flat_map(|shard| &shard.packages).find(|l| l.id == id)
}

pub(crate) fn available<'a>(snap: &Snapshot, listing: &'a Listing) -> Vec<&'a Release> {
    listing.versions.iter().filter(|r| snap.revoked.version(&listing.id, &r.version).is_none()).collect()
}

pub(crate) fn newer(a: &str, b: &str) -> bool {
    Version::parse(a).ok() > Version::parse(b).ok()
}

fn matches(query: &str, id: &str, release: &Release) -> bool {
    let texts = [&release.title, &release.description].into_iter().flat_map(|t| match t {
        Text::Plain(text) => vec![text.as_str()],
        Text::Localized(map) => map.values().map(String::as_str).collect(),
    });
    let lists = release.tags.iter().chain(&release.authors).chain(&release.facets).map(String::as_str);
    let fields = [id].into_iter().chain(texts).chain(lists);
    query.is_empty() || fields.map(str::to_lowercase).any(|f| f.contains(query))
}

#[cfg(test)]
mod tests {
    use std::fs::{File, TryLockError};
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

    static SEEN: AtomicUsize = AtomicUsize::new(0);

    fn held(data: &Path) {
        let file = File::open(data.join(".repos.lock")).unwrap();
        assert!(matches!(file.try_lock(), Err(TryLockError::WouldBlock)), "the scan ran without the lock");
        SEEN.fetch_add(1, Relaxed);
    }

    #[test]
    fn browsing_reads_the_repositories_and_the_installed_list_under_one_lock() {
        let dir = tempfile::tempdir().unwrap();
        crate::store::locked(dir.path(), true, || Ok(())).unwrap();
        crate::broken::SCANNED.with(|hook| hook.set(Some(held)));
        assert!(super::browse(dir.path(), "").unwrap().is_empty());
        assert!(crate::repos(dir.path(), 0).unwrap().is_empty());
        crate::broken::SCANNED.with(|hook| hook.set(None));
        assert_eq!(SEEN.load(Relaxed), 2);
        let file = File::open(dir.path().join(".repos.lock")).unwrap();
        assert!(file.try_lock().is_ok(), "the lock outlived the snapshot");
    }
}
