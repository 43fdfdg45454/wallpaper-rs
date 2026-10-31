use crate::fetch::{bases, local};
use crate::repo::hex64;
use crate::store::{Snapshot, State, json, load, repo_dir};
use crate::update::refresh;
use crate::{Anchor, Error, Fetch, RepoInfo, Result, Root, ensure};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

pub(crate) type Found = Vec<(String, Result<Snapshot>)>;

#[cfg(test)]
thread_local! {
    pub(crate) static SCANNED: std::cell::Cell<Option<fn(&Path)>> = const { std::cell::Cell::new(None) };
}

pub(crate) fn scan(data: &Path) -> Result<Found> {
    #[cfg(test)]
    SCANNED.with(|hook| hook.get().into_iter().for_each(|hook| hook(data)));
    let dirs = match fs::read_dir(data.join("repos")) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        dirs => dirs?,
    };
    let mut names =
        dirs.map(|d| Ok(d?.file_name().to_string_lossy().into_owned())).collect::<Result<Vec<_>>>()?;
    names.retain(|name| hex64(name));
    names.sort();
    Ok(names.into_iter().map(|name| (name.clone(), load(data, &name))).collect())
}

pub(crate) fn sound(found: Found) -> Vec<Snapshot> {
    found.into_iter().filter_map(|(_, snap)| snap.ok()).collect()
}

pub(crate) fn wreck(data: &Path, repo_id: &str, error: &Error) -> RepoInfo {
    let dir = data.join("repos").join(repo_id);
    let root: Option<Root> = json(&dir.join("root.json")).ok();
    let state = seed(&dir, repo_id, root.as_ref());
    let hosts = root.as_ref().map_or_else(|| state.bases.clone(), |root| bases(&state, root));
    RepoInfo {
        repo_id: repo_id.into(),
        url: state.url.clone(),
        name: root.as_ref().map(|r| r.name.clone()).unwrap_or_default(),
        hosts,
        fingerprints: root.map(|r| r.roles.root.keys).unwrap_or_default(),
        root_version: state.root_version,
        entry_version: state.entry_version,
        broken: Some(error.to_string()),
        ..RepoInfo::default()
    }
}

pub(crate) fn recover(data: &Path, fetch: &dyn Fetch, repo_id: &str, now: i64) -> Result<Snapshot> {
    let dir = repo_dir(data, repo_id)?;
    let state = seed(&dir, repo_id, json::<Root>(&dir.join("root.json")).ok().as_ref());
    let lost = || Error::Missing(format!("the address of repository {repo_id}; add it again or remove it"));
    ensure(!state.bases.is_empty(), lost)?;
    let (anchor, floor) = (state.bases[0].clone(), state.root_version);
    let anchor = Anchor { base: anchor, fingerprint: None, repo_id: Some(repo_id.into()) };
    let snap = refresh(fetch, state, None, &BTreeMap::new(), Some(&anchor), now)?;
    let older = || {
        Error::Rollback(format!(
            "root {} is older than the root {floor} seen before",
            snap.root.value.version
        ))
    };
    ensure(snap.root.value.version >= floor, older).map(|()| snap)
}

fn seed(dir: &Path, repo_id: &str, root: Option<&Root>) -> State {
    let kept = json::<State>(&dir.join("state.json")).ok().filter(|s| s.repo_id == repo_id);
    kept.filter(|s| !s.bases.is_empty()).unwrap_or_else(|| {
        let mirrors = root.map_or(&[][..], |r| &r.mirrors[..]).iter();
        let bases: Vec<String> =
            mirrors.filter(|m| !local(m)).map(|m| format!("{}/", m.trim_end_matches('/'))).collect();
        let url = bases.first().map(|b| format!("{b}?repo_id={repo_id}")).unwrap_or_default();
        State { url, bases, repo_id: repo_id.into(), ..State::default() }
    })
}
