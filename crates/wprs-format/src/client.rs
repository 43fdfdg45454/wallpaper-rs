use crate::broken::{recover, scan, wreck};
use crate::browse::{available, listing, newer};
use crate::fetch::bases;
use crate::install::{records, save};
use crate::store::{Snapshot, State, commit, load, locked, repo_dir};
use crate::update::refresh;
use crate::{Anchor, Error, Fetch, Installed, Result, Time, ensure, sha256_file};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

const WARN: i64 = 72 * 3600;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct RepoInfo {
    pub repo_id: String,
    pub url: String,
    pub name: String,
    pub hosts: Vec<String>,
    pub fingerprints: Vec<String>,
    pub root_version: u64,
    pub root_expires: Time,
    pub entry_version: u64,
    pub entry_expires: Time,
    pub checked: Time,
    pub stale: bool,
    pub packages: usize,
    pub warnings: Vec<String>,
    pub broken: Option<String>,
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
    let anchor = Anchor::parse(url)?;
    anchor.repo_id.as_ref().map_or(Ok(()), |id| absent(data, id))?;
    let snap = contact(fetch, url, &anchor, now)?;
    locked(data, true, || {
        absent(data, &snap.state.repo_id)?;
        commit(data, &snap)?;
        Ok(apply(data, &snap, now)?.repo)
    })
}

pub fn update_repo<F: Fetch>(data: &Path, fetch: &F, repo_id: &str, now: i64) -> Result<Outcome> {
    let gone = || Error::NotFound(format!("repository {repo_id}"));
    let old = locked(data, false, || Ok(repo_dir(data, repo_id)?.is_dir().then(|| load(data, repo_id))))?;
    let snap = match old.ok_or_else(gone)? {
        Ok(old) => refresh(fetch, old.state, Some(old.root), &old.blobs, None, now)?,
        Err(_) => recover(data, fetch, repo_id, now)?,
    };
    locked(data, true, || {
        ensure(repo_dir(data, repo_id)?.is_dir(), gone)?;
        if let Ok(current) = load(data, repo_id) {
            let (new, old) = (&snap.state, &current.state);
            let order = |s: &State| (s.root_version, s.entry_version);
            if order(new) < order(old) {
                return apply(data, &current, now);
            }
            let same = order(new) != order(old) || new.entry_sha256 == old.entry_sha256;
            ensure(same, || Error::Equivocation(new.entry_version))?;
        }
        commit(data, &snap)?;
        apply(data, &snap, now)
    })
}

pub fn repos(data: &Path, now: i64) -> Result<Vec<RepoInfo>> {
    locked(data, false, || {
        let found = scan(data)?.into_iter();
        Ok(found.map(|(id, snap)| snap.map_or_else(|e| wreck(data, &id, &e), |s| info(&s, now))).collect())
    })
}

pub fn remove_repo(data: &Path, repo_id: &str) -> Result<()> {
    locked(data, true, || Ok(fs::remove_dir_all(repo_dir(data, repo_id)?)?))
}

fn absent(data: &Path, repo_id: &str) -> Result<()> {
    let found = repo_dir(data, repo_id)?.is_dir() && load(data, repo_id).is_ok();
    ensure(!found, || Error::Exists(format!("repository {repo_id}")))
}

fn contact(fetch: &dyn Fetch, url: &str, anchor: &Anchor, now: i64) -> Result<Snapshot> {
    let state = State { url: url.into(), bases: vec![anchor.base.clone()], ..State::default() };
    refresh(fetch, state, None, &BTreeMap::new(), Some(anchor), now)
}

fn info(snap: &Snapshot, now: i64) -> RepoInfo {
    let (root, entry, state) = (&snap.root.value, &snap.entry.value, &snap.state);
    let left = |expires: &Time| expires.timestamp() - now.max(state.max_timestamp);
    let (root_left, entry_left) = (left(&root.expires), left(&entry.expires));
    let notes = [
        (root_left <= 0, "root metadata has expired: nothing can be installed or updated from it"),
        (root_left > 0 && root_left < WARN, "root metadata expires within 72 hours"),
        (entry_left <= 0, "signed index expired: the repository is stale"),
        (entry_left > 0 && entry_left < WARN, "signed index expires within 72 hours"),
    ];
    RepoInfo {
        repo_id: state.repo_id.clone(),
        url: state.url.clone(),
        name: root.name.clone(),
        hosts: bases(state, root),
        fingerprints: root.roles.root.keys.clone(),
        root_version: root.version,
        root_expires: root.expires,
        entry_version: entry.version,
        entry_expires: entry.expires,
        checked: Time::from_timestamp(state.checked, 0).unwrap_or_default(),
        stale: entry_left <= 0,
        packages: snap.shards.iter().map(|s| s.packages.len()).sum(),
        warnings: notes.iter().filter(|(on, _)| *on).map(|(_, note)| note.to_string()).collect(),
        broken: None,
    }
}

fn apply(data: &Path, snap: &Snapshot, now: i64) -> Result<Outcome> {
    let (mut list, repo_id) = (records(data)?, &snap.state.repo_id);
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
