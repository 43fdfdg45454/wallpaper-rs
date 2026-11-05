use crate::{Error, library, pack, repos, store};
use semver::Version;
use serde::Deserialize;
use serde_json::{Value, json};
use std::fs;

const REFRESHED: &str = "refreshed";
const DAY: i64 = 86_400;

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub(crate) enum Request {
    Updates,
    RefreshRepos { force: bool },
    Update { uid: String },
}

pub(crate) fn request(request: Request) -> Result<Value, Error> {
    match request {
        Request::Updates => list(),
        Request::RefreshRepos { force } => refresh(force),
        Request::Update { uid } => update(&uid),
    }
}

pub(crate) struct Kept {
    pub(crate) uid: String,
    pub(crate) repo: String,
    pub(crate) id: String,
    pub(crate) version: String,
    pub(crate) title: String,
}

pub(crate) fn newer(found: &[wprs_format::Item], kept: &[Kept]) -> Vec<Value> {
    let update = |k: &Kept| {
        let have = Version::parse(&k.version).ok()?;
        let offered = found.iter().filter(|i| i.id == k.id && i.repo_id == k.repo);
        let versions = offered.flat_map(|i| i.versions.iter().chain([&i.release.version]));
        let best = versions.filter_map(|v| Version::parse(v).ok()).max().filter(|best| *best > have)?;
        Some(json!({ "uid": k.uid, "repo_id": k.repo, "id": k.id, "title": k.title,
            "installed": k.version, "available": best.to_string() }))
    };
    kept.iter().filter_map(update).collect()
}

fn kept() -> Result<Vec<Kept>, Error> {
    let installed = repos::installed();
    library::with(|lib| {
        let kept = installed.into_iter().filter_map(|(uid, (repo, id))| {
            let item = lib.item(&uid).ok()?;
            Some(Kept { version: item.package?.version, title: item.title, uid, repo, id })
        });
        Ok(kept.collect())
    })
}

fn list() -> Result<Value, Error> {
    let dir = repos::dir()?;
    let found = if dir.is_dir() { wprs_format::browse_with(&dir, "", true)? } else { Vec::new() };
    Ok(json!(newer(&found, &kept()?)))
}

fn refresh(force: bool) -> Result<Value, Error> {
    let (dir, now) = (repos::dir()?, library::now());
    let last = fs::read_to_string(dir.join(REFRESHED)).ok().and_then(|t| t.trim().parse::<i64>().ok());
    let known = if dir.is_dir() { wprs_format::repos(&dir, now)? } else { Vec::new() };
    if known.is_empty() || (!force && last.is_some_and(|at| (0..DAY).contains(&(now - at)))) {
        return list();
    }
    let fetch = repos::fetch()?;
    for repo in known.iter().filter(|r| r.broken.is_none()) {
        if let Err(e) = wprs_format::update_repo(&dir, &fetch, &repo.repo_id, now) {
            log::warn!("cannot refresh a repository: {e}");
        }
    }
    store::write(&dir.join(REFRESHED), now.to_string().as_bytes())?;
    store::changed();
    list()
}

fn update(uid: &str) -> Result<Value, Error> {
    let Some((repo, id)) = repos::installed().remove(uid) else { return Ok(json!({ "item": null })) };
    let old = library::with(|lib| Ok(lib.item(uid)?))?;
    let (fresh, path) = repos::fetched(&format!("{repo}/{id}"))?;
    if fresh != uid {
        let path = library::with(|lib| {
            lib.remove_items(&[&fresh])?;
            lib.relink_item(uid, &path, library::now())?;
            Ok(lib.item(uid)?.path)
        })?;
        crate::settings::relinked(&old.path, &path)?;
        let mut installed = repos::installed();
        installed.remove(&fresh);
        store::write(&repos::dir()?.join(repos::ITEMS), &serde_json::to_vec(&installed)?)?;
        drop(fs::remove_dir_all(pack::folder(&old.path)?));
        let media = fs::canonicalize(store::dir()?.join(store::MEDIA))?;
        let (before, after) = (fs::canonicalize(&old.path).ok(), fs::canonicalize(&path).ok());
        if let Some(before) = before.filter(|b| b.starts_with(&media) && Some(b) != after.as_ref()) {
            drop(fs::remove_file(before));
        }
        for extension in ["jpg", "colors"] {
            drop(fs::remove_file(store::thumb_file(uid, extension)?));
        }
    }
    crate::variant::needed(&repo, &path, 0);
    store::changed();
    Ok(json!({ "item": uid }))
}
