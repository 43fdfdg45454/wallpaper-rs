use crate::browse::{available, listing, newer};
use crate::client::snapshots;
use crate::download::package;
use crate::store::{Snapshot, locked, optional, write};
use crate::{Error, Fetch, Listing, Reason, Release, Result, ensure, require};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Installed {
    pub repo_id: String,
    pub id: String,
    pub version: String,
    pub sha256: String,
    pub size: u64,
    pub path: PathBuf,
    pub signer: Option<String>,
    pub revoked: Option<Reason>,
}

pub fn install<F: Fetch>(
    data: &Path,
    fetch: &F,
    spec: &str,
    now: i64,
    allow_stale: bool,
    allow_downgrade: bool,
) -> Result<Installed> {
    let (repo, rest) = spec.rsplit_once('/').map_or((None, spec), |(r, s)| (Some(r), s));
    let (id, version) = rest.split_once('@').map_or((rest, None), |(i, v)| (i, Some(v)));
    let offered = |s: &Snapshot| repo.is_none_or(|r| r == s.state.repo_id) && pick(s, id, version).is_some();
    let mut found: Vec<Snapshot> = snapshots(data)?.into_iter().filter(offered).collect();
    require(found.len() <= 1, "id", "is offered by several repositories; prefix it with the repository id")?;
    let snap = found.pop().ok_or_else(|| Error::NotFound(spec.into()))?;
    let (listing, release) = pick(&snap, id, version).ok_or_else(|| Error::NotFound(spec.into()))?;
    let (root, repo_id, t) = (&snap.root.value, &snap.state.repo_id, now.max(snap.state.max_timestamp));
    let foreign = |list: &[Installed]| list.iter().any(|i| i.id == id && &i.repo_id != repo_id);
    let list = installed(data)?;
    require(!foreign(&list), "id", "is installed from another repository")?;
    ensure(root.expires.timestamp() > t, || Error::Expired(format!("root {}", root.version)))?;
    ensure(allow_stale || snap.entry.value.expires.timestamp() > t, || Error::Stale(repo_id.clone()))?;
    if let Some(reason) = snap.revoked.version(id, &release.version) {
        return Err(Error::Revoked(format!("{id} {} ({reason:?})", release.version)));
    }
    let newest = list.iter().filter(|i| i.id == id).max_by_key(|i| Version::parse(&i.version).ok());
    let explicit = version.is_some();
    let older = newest.filter(|n| explicit && !allow_downgrade && newer(&n.version, &release.version));
    if let Some((new, old)) = older.map(|n| (&n.version, &release.version)) {
        let reason = format!("{id} {old} is older than the installed {new}; confirm the downgrade");
        return Err(Error::Invalid { field: "version".into(), reason });
    }
    let newest = newest.filter(|i| !explicit && !newer(&release.version, &i.version));
    if let Some(done) = list.iter().find(|i| i.id == id && i.version == release.version).or(newest) {
        return Ok(done.clone());
    }
    let path = data.join("packages").join(id).join(format!("{}.wpak", release.version));
    let (tmp, signer) = package(fetch, &snap, listing, release, &path)?;
    let (version, sha256, size) = (release.version.clone(), release.sha256.clone(), release.size);
    let (repo_id, id) = (repo_id.clone(), id.to_string());
    let done = Installed { repo_id, id, version, sha256, size, path, signer, revoked: None };
    locked(data, true, || {
        let mut list = records(data)?;
        if let Some(same) = list.iter().find(|i| i.id == done.id && i.version == done.version) {
            return Ok(same.clone());
        }
        require(!foreign(&list), "id", "is installed from another repository")?;
        tmp.keep(&done.path)?;
        list.push(done.clone());
        save(data, &list)?;
        Ok(done)
    })
}

pub fn remove(data: &Path, id: &str, version: Option<&str>) -> Result<Vec<Installed>> {
    locked(data, true, || {
        let (gone, kept): (Vec<_>, Vec<_>) =
            records(data)?.into_iter().partition(|i| i.id == id && version.is_none_or(|v| v == i.version));
        ensure(!gone.is_empty(), || Error::NotFound(id.into()))?;
        for item in &gone {
            match fs::remove_file(&item.path) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e.into()),
                _ => {}
            }
        }
        gone.iter().filter_map(|i| i.path.parent()).for_each(|dir| drop(fs::remove_dir(dir)));
        save(data, &kept)?;
        Ok(gone)
    })
}

pub fn installed(data: &Path) -> Result<Vec<Installed>> {
    locked(data, false, || records(data))
}

pub(crate) fn records(data: &Path) -> Result<Vec<Installed>> {
    optional(&data.join("installed.json"))
}

pub(crate) fn save(data: &Path, list: &[Installed]) -> Result<()> {
    write(&data.join("installed.json"), &serde_json::to_vec_pretty(list)?, true)
}

fn pick<'a>(snap: &'a Snapshot, id: &str, version: Option<&str>) -> Option<(&'a Listing, &'a Release)> {
    let listing = listing(snap, id)?;
    let release = match version {
        Some(version) => listing.versions.iter().find(|r| r.version == version),
        None => available(snap, listing).pop(),
    };
    Some((listing, release?))
}
