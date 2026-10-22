use crate::browse::{available, listing, newer};
use crate::install::save;
use crate::store::{Snapshot, State, commit, load, repo_dir};
use crate::update::refresh;
use crate::{Anchor, Fetch, Installed, Result, Time, installed, require, sha256_file};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

const WARN: i64 = 72 * 3600;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RepoInfo {
    pub repo_id: String,
    pub url: String,
    pub name: String,
    pub root_version: u64,
    pub root_expires: Time,
    pub entry_version: u64,
    pub entry_expires: Time,
    pub stale: bool,
    pub packages: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Update {
    pub id: String,
    pub installed: String,
    pub available: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Outcome {
    pub repo: RepoInfo,
    pub updates: Vec<Update>,
    pub revoked: Vec<Installed>,
}

pub fn verify<F: Fetch>(fetch: &F, url: &str, now: i64) -> Result<RepoInfo> {
    let anchor = match Anchor::parse(url) {
        Err(_) if !url.contains('?') => {
            let dir = Path::new(url.strip_prefix("file://").unwrap_or(url));
            let repo_id = Some(sha256_file(dir.join("root/1.json"))?);
            Anchor { base: format!("{}/", url.trim_end_matches('/')), fingerprint: None, repo_id }
        }
        anchor => anchor?,
    };
    Ok(info(&contact(fetch, url, &anchor, now)?, now))
}

pub fn add_repo<F: Fetch>(data: &Path, fetch: &F, url: &str, now: i64) -> Result<RepoInfo> {
    let snap = contact(fetch, url, &Anchor::parse(url)?, now)?;
    let fresh = !repo_dir(data, &snap.state.repo_id)?.join("state.json").exists();
    require(fresh, "url", "this repository is already added")?;
    commit(data, &snap)?;
    Ok(apply(data, &snap, now)?.repo)
}

pub fn update_repo<F: Fetch>(data: &Path, fetch: &F, repo_id: &str, now: i64) -> Result<Outcome> {
    let old = load(data, repo_id)?;
    let snap = refresh(fetch, old.state, Some(old.root), &old.blobs, None, now)?;
    commit(data, &snap)?;
    apply(data, &snap, now)
}

pub fn repos(data: &Path) -> Result<Vec<RepoInfo>> {
    Ok(snapshots(data)?.iter().map(|snap| info(snap, snap.state.checked)).collect())
}

pub fn remove_repo(data: &Path, repo_id: &str) -> Result<()> {
    Ok(fs::remove_dir_all(repo_dir(data, repo_id)?)?)
}

pub(crate) fn snapshots(data: &Path) -> Result<Vec<Snapshot>> {
    let dirs = match fs::read_dir(data.join("repos")) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        dirs => dirs?,
    };
    let names = dirs.map(|d| Ok(d?.file_name().to_string_lossy().into_owned()));
    let names = names.collect::<Result<Vec<_>>>()?;
    names.iter().filter(|n| !n.starts_with('.')).map(|name| load(data, name)).collect()
}

fn contact(fetch: &dyn Fetch, url: &str, anchor: &Anchor, now: i64) -> Result<Snapshot> {
    let state = State { url: url.into(), bases: vec![anchor.base.clone()], ..State::default() };
    refresh(fetch, state, None, &BTreeMap::new(), Some(anchor), now)
}

fn info(snap: &Snapshot, now: i64) -> RepoInfo {
    let (root, entry, state) = (&snap.root.value, &snap.entry.value, &snap.state);
    let soon = |expires: &Time| expires.timestamp() - now < WARN;
    let notes = [
        (soon(&root.expires), "root metadata expires within 72 hours"),
        (state.stale, "signed index expired: the repository is stale"),
        (!state.stale && soon(&entry.expires), "signed index expires within 72 hours"),
    ];
    RepoInfo {
        repo_id: state.repo_id.clone(),
        url: state.url.clone(),
        name: root.name.clone(),
        root_version: root.version,
        root_expires: root.expires,
        entry_version: entry.version,
        entry_expires: entry.expires,
        stale: state.stale,
        packages: snap.shards.iter().map(|s| s.packages.len()).sum(),
        warnings: notes.iter().filter(|(on, _)| *on).map(|(_, note)| note.to_string()).collect(),
    }
}

fn apply(data: &Path, snap: &Snapshot, now: i64) -> Result<Outcome> {
    let (mut list, repo_id) = (installed(data)?, &snap.state.repo_id);
    for item in list.iter_mut().filter(|i| &i.repo_id == repo_id) {
        let key = item.signer.as_deref().and_then(|fp| snap.revoked.key(fp));
        item.revoked = snap.revoked.version(&item.id, &item.version).or(key);
    }
    save(data, &list)?;
    let mut newest: BTreeMap<&str, &Installed> = BTreeMap::new();
    for item in list.iter().filter(|i| &i.repo_id == repo_id) {
        let slot = newest.entry(&item.id).or_insert(item);
        if newer(&item.version, &slot.version) {
            *slot = item;
        }
    }
    let updates = newest.values().filter_map(|item| {
        let latest = available(snap, listing(snap, &item.id)?).pop()?;
        let (id, installed, available) = (item.id.clone(), item.version.clone(), latest.version.clone());
        newer(&latest.version, &item.version).then_some(Update { id, installed, available })
    });
    let revoked = list.iter().filter(|i| &i.repo_id == repo_id && i.revoked.is_some()).cloned().collect();
    Ok(Outcome { repo: info(snap, now), updates: updates.collect(), revoked })
}
