use crate::library::with;
use crate::{Error, pack, props, repos, said, store};
use semver::{Version, VersionReq};
use serde_json::json;
use std::path::Path;
use wprs_format::{Dependency, Limits};
use wprs_library::{Item, Library};

const DEPTH: u32 = 4;

fn version(item: &Item) -> Option<Version> {
    Version::parse(&item.package.as_ref()?.version).ok()
}

fn wanted(dependency: &Dependency) -> Result<VersionReq, Error> {
    VersionReq::parse(&dependency.version).map_err(|e| {
        said("android.version_range", "%1 is not a version range: %2", &[&dependency.version, &e])
    })
}

fn present(lib: &Library, dependency: &Dependency) -> Result<Option<Item>, Error> {
    let wanted = wanted(dependency)?;
    let usable = |i: &Item| {
        i.package.as_ref().is_some_and(|p| p.kind != "preset" && p.kind != "assets")
            && version(i).is_some_and(|v| wanted.matches(&v))
    };
    Ok(lib.packages_of(&dependency.id)?.into_iter().filter(usable).max_by_key(version))
}

pub(crate) fn preset(lib: &Library, uid: &str) -> Result<Option<String>, Error> {
    let item = lib.item(uid)?;
    if item.package.as_ref().is_none_or(|p| p.kind != "preset") {
        return Ok(None);
    }
    let base = wprs_format::open(&item.path, &Limits::phone())?.preset()?.base;
    let found = present(lib, &base)?;
    let needs = || {
        said("android.preset_needs", "this preset needs %1 %2; install it first", &[&base.id, &base.version])
    };
    found.map(|base| Some(base.uid)).ok_or_else(needs)
}

pub(crate) fn apply(uid: &str) -> Result<String, Error> {
    let Some(base) = with(|lib| preset(lib, uid))? else { return Ok(uid.to_owned()) };
    let path = with(|lib| Ok(lib.item(uid)?.path))?;
    let mut package = wprs_format::open(&path, &Limits::phone())?;
    let preset = package.preset()?;
    let pack = pack::load(&with(|lib| Ok(lib.item(&base)?.path))?)?;
    let chosen = wprs_format::values::chosen(&preset, &pack.properties);
    chosen.warnings.iter().for_each(|w| log::warn!("{}: {w}", package.manifest.id));
    let mut values = chosen.values;
    let dir = store::dir()?.join("presets").join(&package.manifest.id);
    for (id, file) in chosen.files {
        let ext = Path::new(&file).extension().and_then(|e| e.to_str()).unwrap_or("bin");
        let kept = dir.join(format!("{id}.{ext}"));
        std::fs::create_dir_all(&dir)?;
        store::write(&kept, &package.read_file(&file)?)?;
        values.insert(id, json!(kept));
    }
    props::change(&pack, |kept| {
        kept.extend(values.iter().filter_map(|(k, v)| Some((k.clone(), props::stored(v)?))))
    })?;
    Ok(base)
}

pub(crate) fn needed(repo: &str, path: &Path, depth: u32) -> Vec<String> {
    let needs = wprs_format::peek(path).map(|m| m.dependencies().to_vec()).unwrap_or_default();
    let mut installed = Vec::new();
    for dependency in needs.iter().filter(|d| !with(|lib| present(lib, d)).is_ok_and(|found| found.is_some()))
    {
        let done = matching(repo, dependency).and_then(|spec| repos::fetched(&spec));
        match done {
            Ok((uid, path)) if depth < DEPTH => {
                installed.extend(needed(repo, &path, depth + 1));
                installed.push(uid);
            }
            Ok((uid, _)) => installed.push(uid),
            Err(e) => log::warn!("cannot install {} that a package needs: {e}", dependency.id),
        }
    }
    installed
}

fn matching(repo: &str, dependency: &Dependency) -> Result<String, Error> {
    let wanted = wanted(dependency)?;
    let offers = wprs_format::browse_with(&repos::dir()?, &dependency.id, true)?;
    let offer = offers.into_iter().find(|o| o.id == dependency.id && o.repo_id == repo);
    let versions = offer.map(|o| o.versions).unwrap_or_default();
    let best = versions.iter().filter_map(|v| Version::parse(v).ok()).filter(|v| wanted.matches(v)).max();
    let missing =
        || said("android.no_release", "no release of %1 matches %2", &[&dependency.id, &dependency.version]);
    best.map(|v| format!("{repo}/{}@{v}", dependency.id)).ok_or_else(missing)
}
