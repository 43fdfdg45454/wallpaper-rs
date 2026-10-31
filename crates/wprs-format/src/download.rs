use crate::browse::listing;
use crate::fetch::{bases, first};
use crate::store::{Snapshot, Temp, load, locked, place};
use crate::{
    Error, Fetch, Limits, Listing, MANIFEST, OnlineAuthor, Release, Result, ensure, read, sha256,
    verify_package,
};
use std::fs::File;
use std::io::{BufReader, Seek};
use std::path::Path;

pub fn preview<F: Fetch>(
    data: &Path,
    fetch: &F,
    repo_id: &str,
    id: &str,
    version: &str,
    dest: &Path,
) -> Result<bool> {
    let snap = locked(data, false, || load(data, repo_id))?;
    let Some(preview) = &found(&snap, id, version)?.preview else { return Ok(false) };
    let remote = format!("previews/{}.{}", preview.sha256, preview.ext);
    place(dest, true, |file| verified(fetch, &snap, &remote, preview.size, &preview.sha256, file))?;
    Ok(true)
}

pub fn release(data: &Path, repo_id: &str, id: &str, version: &str) -> Result<Release> {
    let snap = locked(data, false, || load(data, repo_id))?;
    found(&snap, id, version).cloned()
}

pub fn previews(data: &Path, repo_id: &str) -> Result<Vec<String>> {
    let snap = locked(data, false, || load(data, repo_id))?;
    let releases = snap.shards.iter().flat_map(|s| &s.packages).flat_map(|l| &l.versions);
    let mut shas: Vec<String> = releases.filter_map(|r| Some(r.preview.as_ref()?.sha256.clone())).collect();
    shas.sort();
    shas.dedup();
    Ok(shas)
}

fn found<'a>(snap: &'a Snapshot, id: &str, version: &str) -> Result<&'a Release> {
    let release = listing(snap, id).and_then(|l| l.versions.iter().find(|r| r.version == version));
    release.ok_or_else(|| Error::NotFound(format!("{id}@{version}")))
}

pub(crate) fn package(
    fetch: &dyn Fetch,
    snap: &Snapshot,
    listing: &Listing,
    release: &Release,
    path: &Path,
) -> Result<(Temp, Option<String>, Option<OnlineAuthor>)> {
    let mut tmp = Temp::new(path, true)?;
    let remote = format!("packages/{}/{}.wpak", listing.id, release.version);
    verified(fetch, snap, &remote, release.size, &release.sha256, &mut tmp.file)?;
    let (signer, online) = checked(&tmp.file, snap, listing, release)?;
    Ok((tmp, signer, online))
}

fn verified(
    fetch: &dyn Fetch,
    snap: &Snapshot,
    remote: &str,
    size: u64,
    sha: &str,
    file: &mut File,
) -> Result<()> {
    let fetched = first(&bases(&snap.state, &snap.root.value), |base| {
        file.set_len(0)?;
        file.rewind()?;
        if !fetch.copy(base, remote, size, file)? {
            return Ok(None);
        }
        file.rewind()?;
        let ok = file.metadata()?.len() == size && sha256(&mut *file)? == sha;
        ensure(ok, || Error::Mismatch(remote.into())).map(Some)
    })?;
    fetched.ok_or_else(|| Error::Missing(remote.into()))?;
    Ok(file.rewind()?)
}

type Signed = (Option<String>, Option<OnlineAuthor>);

fn checked(file: &File, snap: &Snapshot, listing: &Listing, release: &Release) -> Result<Signed> {
    let (root, id) = (&snap.root.value, &listing.id);
    let mut package = read(BufReader::new(file), &Limits::default())?;
    let digest = sha256(&package.read_file(MANIFEST)?[..])?;
    let m = &package.manifest;
    let same =
        m.id == *id && m.version == release.version && m.kind == release.kind && m.license == release.license;
    ensure(same && digest == release.manifest_sha256, || Error::Mismatch(format!("manifest of {id}")))?;
    if !root.policy.require_author_signature && listing.author.is_none() {
        return Ok((None, None));
    }
    let (name, revoked) = (listing.author.as_deref(), &snap.revoked.keys);
    if let Some(author) = name.and_then(|a| root.authors.iter().find(|x| x.id == a)) {
        return Ok((Some(verify_package(&mut package, &root.author_keys(author, revoked))?), None));
    }
    let online = name.filter(|_| root.policy.online()).and_then(|a| snap.authors.as_ref()?.find(a));
    let author = online.ok_or_else(|| Error::Signature(format!("{id} has no author bound in the root")))?;
    Ok((Some(verify_package(&mut package, &author.material(revoked))?), Some(author.clone())))
}
