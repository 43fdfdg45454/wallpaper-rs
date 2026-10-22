use crate::browse::{available, listing, newer};
use crate::client::snapshots;
use crate::fetch::{bases, first};
use crate::store::{Snapshot, optional, place, write};
use crate::{
    Error, Fetch, Limits, Listing, MANIFEST, Reason, Release, Result, ensure, read, require, sha256,
    verify_package,
};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, BufReader, Seek};
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
) -> Result<Installed> {
    let (repo, rest) = spec.rsplit_once('/').map_or((None, spec), |(r, s)| (Some(r), s));
    let (id, version) = rest.split_once('@').map_or((rest, None), |(i, v)| (i, Some(v)));
    let offered = |s: &Snapshot| repo.is_none_or(|r| r == s.state.repo_id) && listing(s, id).is_some();
    let mut found: Vec<Snapshot> = snapshots(data)?.into_iter().filter(offered).collect();
    require(found.len() <= 1, "id", "is offered by several repositories; prefix it with the repository id")?;
    let snap = found.pop().ok_or_else(|| Error::NotFound(spec.into()))?;
    let listing = listing(&snap, id).ok_or_else(|| Error::NotFound(spec.into()))?;
    let release = match version {
        Some(version) => listing.versions.iter().find(|r| r.version == version),
        None => available(&snap, listing).pop(),
    };
    let release = release.ok_or_else(|| Error::NotFound(spec.into()))?;
    let (root, repo_id, t) = (&snap.root.value, &snap.state.repo_id, now.max(snap.state.max_timestamp));
    ensure(root.expires.timestamp() > t, || Error::Expired(format!("root {}", root.version)))?;
    ensure(allow_stale || snap.entry.value.expires.timestamp() > t, || Error::Stale(repo_id.clone()))?;
    if let Some(reason) = snap.revoked.version(id, &release.version) {
        return Err(Error::Revoked(format!("{id} {} ({reason:?})", release.version)));
    }
    let mut list = installed(data)?;
    let foreign = list.iter().any(|i| i.id == id && &i.repo_id != repo_id);
    require(!foreign, "id", "is installed from another repository")?;
    let newest = list.iter().filter(|i| i.id == id).max_by_key(|i| Version::parse(&i.version).ok());
    let newest = newest.filter(|i| version.is_none() && !newer(&release.version, &i.version));
    if let Some(done) = list.iter().find(|i| i.id == id && i.version == release.version).or(newest) {
        return Ok(done.clone());
    }
    let path = data.join("packages").join(id).join(format!("{}.wpak", release.version));
    let signer = download(fetch, &snap, listing, release, &path)?;
    let (version, sha256, size) = (release.version.clone(), release.sha256.clone(), release.size);
    let done = Installed {
        repo_id: repo_id.clone(),
        id: id.into(),
        version,
        sha256,
        size,
        path,
        signer,
        revoked: None,
    };
    list.push(done.clone());
    save(data, &list)?;
    Ok(done)
}

pub fn remove(data: &Path, id: &str, version: Option<&str>) -> Result<Vec<Installed>> {
    let all = installed(data)?;
    let (gone, kept): (Vec<_>, Vec<_>) =
        all.into_iter().partition(|i| i.id == id && version.is_none_or(|v| v == i.version));
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
}

pub fn installed(data: &Path) -> Result<Vec<Installed>> {
    optional(&data.join("installed.json"))
}

pub(crate) fn save(data: &Path, list: &[Installed]) -> Result<()> {
    write(&data.join("installed.json"), &serde_json::to_vec_pretty(list)?, true)
}

fn download(
    fetch: &dyn Fetch,
    snap: &Snapshot,
    listing: &Listing,
    release: &Release,
    path: &Path,
) -> Result<Option<String>> {
    let (root, id) = (&snap.root.value, &listing.id);
    let remote = format!("packages/{id}/{}.wpak", release.version);
    let mut signer = None;
    place(path, true, |file| {
        let fetched = first(&bases(&snap.state, root), |base| {
            file.set_len(0)?;
            file.rewind()?;
            if !fetch.copy(base, &remote, release.size, file)? {
                return Ok(None);
            }
            file.rewind()?;
            let ok = file.metadata()?.len() == release.size && sha256(&mut *file)? == release.sha256;
            ensure(ok, || Error::Mismatch(remote.clone())).map(Some)
        })?;
        fetched.ok_or_else(|| Error::Missing(remote.clone()))?;
        file.rewind()?;
        let mut package = read(BufReader::new(&*file), &Limits::default())?;
        let digest = sha256(&package.read_file(MANIFEST)?[..])?;
        let m = &package.manifest;
        let same = m.id == *id
            && m.version == release.version
            && m.kind == release.kind
            && m.license == release.license;
        ensure(same && digest == release.manifest_sha256, || Error::Mismatch(format!("manifest of {id}")))?;
        if root.policy.require_author_signature || listing.author.is_some() {
            let author = listing.author.as_deref().and_then(|a| root.authors.iter().find(|x| x.id == a));
            let author =
                author.ok_or_else(|| Error::Signature(format!("{id} has no author bound in the root")))?;
            signer = Some(verify_package(&mut package, &root.author_keys(author, &snap.revoked.keys))?);
        }
        Ok(())
    })?;
    Ok(signer)
}
