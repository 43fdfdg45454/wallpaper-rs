use crate::client::snapshots;
use crate::store::Snapshot;
use crate::{Listing, Release, Result, installed};
use semver::Version;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Item {
    pub repo_id: String,
    pub id: String,
    pub author: Option<String>,
    pub release: Release,
    pub versions: Vec<String>,
    pub installed: Vec<String>,
}

pub fn browse(data: &Path, query: &str) -> Result<Vec<Item>> {
    let (query, installed) = (query.to_lowercase(), installed(data)?);
    let mut items = Vec::new();
    for snap in snapshots(data)? {
        for listing in snap.shards.iter().flat_map(|shard| &shard.packages) {
            let open = available(&snap, listing);
            let Some(release) = open.last().filter(|r| matches(&query, &listing.id, r)) else { continue };
            let here = installed.iter().filter(|i| i.repo_id == snap.state.repo_id && i.id == listing.id);
            items.push(Item {
                repo_id: snap.state.repo_id.clone(),
                id: listing.id.clone(),
                author: listing.author.clone(),
                release: (*release).clone(),
                versions: open.iter().map(|r| r.version.clone()).collect(),
                installed: here.map(|i| i.version.clone()).collect(),
            });
        }
    }
    items.sort_by(|a, b| (&a.id, &a.repo_id).cmp(&(&b.id, &b.repo_id)));
    Ok(items)
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
    let lists = release.tags.iter().chain(&release.authors).map(String::as_str);
    let fields = [id, &release.title, &release.description].into_iter().chain(lists);
    query.is_empty() || fields.map(str::to_lowercase).any(|f| f.contains(query))
}
