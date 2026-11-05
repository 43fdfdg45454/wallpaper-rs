use crate::library::with;
use crate::pack::Pack;
use crate::{Error, pack, props, repos, said, store};
use semver::{Version, VersionReq};
use serde_json::json;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError, Weak};
use wprs_format::{Dependency, Limits};
use wprs_library::{Item, Library};
use wprs_scene::{Property, Values};

const DEPTH: u32 = 4;

type Tried = ((String, PathBuf, usize), Weak<Pack>);

static TRIED: Mutex<Vec<Tried>> = Mutex::new(Vec::new());

fn version(item: &Item) -> Option<Version> {
    Version::parse(&item.package.as_ref()?.version).ok()
}

fn wanted(dependency: &Dependency) -> Result<VersionReq, Error> {
    VersionReq::parse(&dependency.version).map_err(|e| {
        said("android.version_range", "%1 is not a version range: %2", &[&dependency.version, &e])
    })
}

pub(crate) fn present(lib: &Library, dependency: &Dependency) -> Result<Option<Item>, Error> {
    let wanted = wanted(dependency)?;
    let usable = |i: &Item| {
        i.package.as_ref().is_some_and(|p| p.kind != "preset" && p.kind != "assets")
            && version(i).is_some_and(|v| wanted.matches(&v))
    };
    Ok(lib.packages_of(&dependency.id)?.into_iter().filter(usable).max_by_key(version))
}

pub(crate) fn based(item: &Item) -> Result<Option<Dependency>, Error> {
    if item.package.as_ref().is_none_or(|p| p.kind != "preset") {
        return Ok(None);
    }
    Ok(Some(wprs_format::open(&item.path, &Limits::phone())?.preset()?.base))
}

pub(crate) fn preset(lib: &Library, uid: &str) -> Result<Option<String>, Error> {
    let Some(base) = based(&lib.item(uid)?)? else { return Ok(None) };
    let needs = || {
        said("android.preset_needs", "this preset needs %1 %2; install it first", &[&base.id, &base.version])
    };
    present(lib, &base)?.map(|base| Some(base.uid)).ok_or_else(needs)
}

fn chosen(uid: &str, properties: &BTreeMap<String, Property>) -> Result<Values, Error> {
    let path = with(|lib| Ok(lib.item(uid)?.path))?;
    let mut package = wprs_format::open(&path, &Limits::phone())?;
    let chosen = wprs_format::values::chosen(&package.preset()?, properties);
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
    Ok(values)
}

pub(crate) fn apply(uid: &str) -> Result<String, Error> {
    let Some(base) = with(|lib| preset(lib, uid))? else { return Ok(uid.to_owned()) };
    let pack = pack::load(&with(|lib| Ok(lib.item(&base)?.path))?)?;
    let values = chosen(uid, &pack.properties)?;
    props::change(&pack, |kept| {
        kept.extend(values.iter().filter_map(|(k, v)| Some((k.clone(), props::stored(v)?))))
    })?;
    Ok(base)
}

pub(crate) fn loaded(path: &Path, preset: Option<&str>) -> Result<Arc<Pack>, Error> {
    let base = pack::load(path)?;
    let Some(preset) = preset else { return Ok(base) };
    let key = (preset.to_owned(), path.to_path_buf(), Arc::as_ptr(&base) as usize);
    let mut tried = TRIED.lock().unwrap_or_else(PoisonError::into_inner);
    tried.retain(|(_, made)| made.strong_count() > 0);
    if let Some(pack) = tried.iter().find(|(k, _)| *k == key).and_then(|(_, made)| made.upgrade()) {
        return Ok(pack);
    }
    let values = chosen(preset, &base.properties)?;
    let pack = Arc::new(Pack { tried: Some(values), ..Pack::clone(&base) });
    tried.push((key, Arc::downgrade(&pack)));
    Ok(pack)
}

pub(crate) fn needed(repo: &str, path: &Path, depth: u32) -> Vec<String> {
    let needs = wprs_format::peek(path).map(|m| m.dependencies().to_vec()).unwrap_or_default();
    let mut installed = Vec::new();
    for dependency in needs.iter().filter(|d| !with(|lib| present(lib, d)).is_ok_and(|found| found.is_some()))
    {
        let done = matching(Some(repo), dependency).and_then(|spec| repos::fetched(&spec));
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

pub(crate) fn matching(repo: Option<&str>, dependency: &Dependency) -> Result<String, Error> {
    let wanted = wanted(dependency)?;
    let offers = wprs_format::browse_with(&repos::dir()?, &dependency.id, true)?;
    let best = offers
        .into_iter()
        .filter(|o| o.id == dependency.id && repo.is_none_or(|r| o.repo_id == r))
        .find_map(|o| {
            let v =
                o.versions.iter().filter_map(|v| Version::parse(v).ok()).filter(|v| wanted.matches(v)).max();
            v.map(|v| format!("{}/{}@{v}", o.repo_id, dependency.id))
        });
    let missing =
        || said("android.no_release", "no release of %1 matches %2", &[&dependency.id, &dependency.version]);
    best.ok_or_else(missing)
}
