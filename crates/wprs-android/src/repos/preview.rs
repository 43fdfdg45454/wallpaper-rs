use super::{dir, fetch, now};
use crate::{Error, settings, store};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const FOLDER: &str = "previews";
const LIMIT: u64 = 16 << 20;

fn folder() -> Result<PathBuf, Error> {
    let folder = store::dir()?.join(FOLDER);
    fs::create_dir_all(&folder)?;
    Ok(folder)
}

pub(crate) fn evict(folder: &Path, limit: u64) {
    let mut files: Vec<(SystemTime, u64, PathBuf)> = fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            Some((meta.modified().ok()?, meta.len(), e.path()))
        })
        .collect();
    files.sort();
    let mut total: u64 = files.iter().map(|f| f.1).sum();
    for (_, size, path) in files {
        if total <= limit {
            break;
        }
        if fs::remove_file(path).is_ok() {
            total -= size;
        }
    }
}

pub(crate) fn thumb(spec: &str) -> Result<Value, Error> {
    let [repo_id, id, version] = spec.splitn(3, '/').collect::<Vec<_>>()[..] else { return Ok(Value::Null) };
    let release = wprs_format::release(&dir()?, repo_id, id, version)?;
    let mature = release.content.as_ref().is_some_and(wprs_format::Rated::mature);
    let Some(preview) = release.preview.filter(|_| !mature || settings::config().library.show_mature) else {
        return Ok(Value::Null);
    };
    let kept = folder()?.join(format!("{}.{}", preview.sha256, preview.ext));
    if !kept.is_file() {
        let hosts = wprs_format::repos(&dir()?, now())?.into_iter().find(|r| r.repo_id == repo_id);
        let fetch = fetch()?.within(&hosts.map(|r| r.hosts).unwrap_or_default());
        let part = kept.with_extension("part");
        let found = wprs_format::preview(&dir()?, &fetch, repo_id, id, version, &part)?;
        if !found {
            return Ok(Value::Null);
        }
        fs::rename(&part, &kept)?;
        evict(&folder()?, LIMIT);
    }
    Ok(json!(kept))
}

pub(crate) fn forget(repo_id: &str) {
    let shas = wprs_format::previews(&dir().unwrap_or_default(), repo_id).unwrap_or_default();
    let Ok(folder) = folder() else { return };
    let named = fs::read_dir(folder).into_iter().flatten().flatten();
    for entry in
        named.filter(|e| shas.iter().any(|sha| e.file_name().to_string_lossy().starts_with(sha.as_str())))
    {
        drop(fs::remove_file(entry.path()));
    }
}
