use crate::authors::trust;
use crate::broken::{scan, sound};
use crate::browse::{available, listing, newer};
use crate::download::package;
use crate::store::{Snapshot, load, locked, optional, write};
use crate::{
    Error, Fetch, Listing, OnlineAuthor, Reason, Release, Result, Trusted, anchors, ensure, require,
};
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
    let (snaps, list) = locked(data, false, || Ok((sound(scan(data)?), records(data)?)))?;
    let mut found: Vec<Snapshot> = snaps.into_iter().filter(offered).collect();
    require(found.len() <= 1, "id", "is offered by several repositories; prefix it with the repository id")?;
    let snap = found.pop().ok_or_else(|| Error::NotFound(spec.into()))?;
    let (listing, release) = pick(&snap, id, version).ok_or_else(|| Error::NotFound(spec.into()))?;
    let (root, repo_id, t) = (&snap.root.value, &snap.state.repo_id, now.max(snap.state.max_timestamp));
    let foreign = |list: &[Installed]| list.iter().any(|i| i.id == id && &i.repo_id != repo_id);
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
    let (tmp, signer, online) = package(fetch, &snap, listing, release, &path)?;
    let (version, sha256, size) = (release.version.clone(), release.sha256.clone(), release.size);
    let (repo_id, id) = (repo_id.clone(), id.to_string());
    let done = Installed { repo_id, id, version, sha256, size, path, signer, revoked: None };
    locked(data, true, || {
        let mut list = records(data)?;
        if let Some(same) = list.iter().find(|i| i.id == done.id && i.version == done.version) {
            return Ok(same.clone());
        }
        require(!foreign(&list), "id", "is installed from another repository")?;
        still(data, &done)?;
        let anchors = trusted(data, &done, &list, online.as_ref(), &snap, listing.author.as_deref())?;
        tmp.keep(&done.path)?;
        list.push(done.clone());
        save(data, &list)?;
        anchors
            .map_or(Ok(()), |a| write(&data.join("anchors.json"), &serde_json::to_vec_pretty(&a)?, true))?;
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

pub fn current(data: &Path, repo_id: &str, id: &str, version: &str) -> Result<Installed> {
    locked(data, false, || {
        let found =
            records(data)?.into_iter().find(|i| i.repo_id == repo_id && i.id == id && i.version == version);
        let found = found.ok_or_else(|| Error::NotFound(format!("{id} {version} was uninstalled")))?;
        if let Some(reason) = found.revoked {
            return Err(Error::Revoked(format!("{id} {version} ({reason:?})")));
        }
        still(data, &found).map(|()| found)
    })
}

fn trusted(
    data: &Path,
    done: &Installed,
    list: &[Installed],
    online: Option<&OnlineAuthor>,
    snap: &Snapshot,
    named: Option<&str>,
) -> Result<Option<Vec<Trusted>>> {
    let (root, revoked) = (&snap.root.value, &snap.revoked.keys);
    let mut anchors = anchors(data)?;
    let same = |r: &str, i: &str| r == done.repo_id && i == done.id;
    let at = anchors.iter().position(|a| same(&a.repo_id, &a.id));
    let signed: Vec<&str> =
        list.iter().filter(|i| same(&i.repo_id, &i.id)).filter_map(|i| i.signer.as_deref()).collect();
    let refuse = |why: &str| Err(Error::Signature(format!("{} {why}", done.id)));
    let owner =
        |fp: &str| root.authors.iter().find(|a| root.author_keys(a, &[]).iter().any(|(f, _)| *f == fp));
    match (done.signer.as_deref(), online) {
        (None, _) if at.is_some() || !signed.is_empty() => {
            refuse("is unsigned, but its installed versions were signed")
        }
        (None, _) => Ok(None),
        (Some(_), Some(_)) if at.is_none() && !signed.is_empty() => {
            refuse("was installed from an author of the root and cannot pass to an online author")
        }
        (Some(fp), Some(author)) => {
            let key = author.keys.get(fp).map(|k| k.minisign.clone()).unwrap_or_default();
            let new =
                Trusted { repo_id: done.repo_id.clone(), id: done.id.clone(), fingerprint: fp.into(), key };
            trust(&mut anchors, new, author, revoked)?;
            Ok(Some(anchors))
        }
        (Some(_), None) if signed.iter().any(|s| owner(s).is_some_and(|o| Some(o.id.as_str()) != named)) => {
            refuse("is signed by another author of the root than its installed versions")
        }
        (Some(_), None) => Ok(at.map(|i| {
            anchors.remove(i);
            anchors
        })),
    }
}

fn still(data: &Path, item: &Installed) -> Result<()> {
    let removed = |_| Error::NotFound(format!("repository {} was removed or broken", item.repo_id));
    let snap = load(data, &item.repo_id).map_err(removed)?;
    let key = item.signer.as_deref().and_then(|fp| snap.revoked.key(fp));
    match snap.revoked.version(&item.id, &item.version).or(key) {
        Some(reason) => Err(Error::Revoked(format!("{} {} ({reason:?})", item.id, item.version))),
        None => Ok(()),
    }
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
