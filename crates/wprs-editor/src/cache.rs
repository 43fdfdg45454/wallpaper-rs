use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use wprs_format::{Error, Folder, Manifest, Scene, Strings};
use wprs_scene::{Descriptor, Issue, Layer, ModelFile, STOPPED, SceneError, UNCHECKED};

type Stamps = Vec<(String, Option<(i64, i64, u64)>)>;
type Read = (BTreeMap<String, Arc<Descriptor>>, BTreeMap<String, Arc<ModelFile>>, Option<Issue>);

type Cache<V> = Mutex<BTreeMap<PathBuf, ((String, Stamps), V)>>;

static TABLES: Cache<Result<Strings, (String, String)>> = Mutex::new(BTreeMap::new());
static FILES: Cache<Read> = Mutex::new(BTreeMap::new());

fn stamps(dir: &Path, paths: impl IntoIterator<Item = String>) -> Stamps {
    let stamp = |m: fs::Metadata| (m.mtime(), m.mtime_nsec(), m.len());
    paths.into_iter().map(|p| (p.clone(), fs::metadata(dir.join(&p)).ok().map(stamp))).collect()
}

fn remembered<V: Clone>(
    cache: &Cache<V>,
    dir: &Path,
    key: (String, Stamps),
    make: impl FnOnce() -> V,
    keep: impl FnOnce(&V) -> bool,
) -> V {
    let mut cache = cache.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((found, value)) = cache.get(dir)
        && *found == key
    {
        return value.clone();
    }
    if cache.len() >= 8 {
        cache.clear();
    }
    let value = make();
    if keep(&value) {
        cache.insert(dir.to_path_buf(), (key, value.clone()));
    } else {
        cache.remove(dir);
    }
    value
}

pub fn with_strings(dir: &Path, manifest: Manifest) -> wprs_format::Result<Manifest> {
    let names = wprs_format::locale_names(dir);
    let shape = serde_json::json!([manifest.schema, manifest.features(), manifest.strings]).to_string();
    let key = (shape, stamps(dir, names));
    let read = || match wprs_format::with_strings(dir, manifest.clone()) {
        Ok(read) => Ok(read.string_files),
        Err(Error::Invalid { field, reason }) => Err((field, reason)),
        Err(e) => Err(("strings".into(), e.to_string())),
    };
    let tables = remembered(&TABLES, dir, key, read, |_| true);
    let string_files = tables.map_err(|(field, reason)| Error::Invalid { field, reason })?;
    Ok(Manifest { string_files, ..manifest })
}

pub fn read_files(scene: &mut Scene, dir: &Path) -> Option<Issue> {
    let figures = scene.layers().chain(scene.templates.values()).filter_map(Layer::figure);
    let models: Vec<(&str, &str)> =
        figures.filter_map(|id| Some((id, scene.assets.get(id)?.path.as_str()))).collect();
    let effects = scene.effects.values().flat_map(|e| [e.descriptor.clone(), e.wgsl.clone()]);
    let paths = effects.chain(models.iter().map(|(_, path)| path.to_string()));
    let shape = serde_json::json!([scene.effects, models]).to_string();
    let key = (shape, stamps(dir, paths));
    let make = || {
        let mut probe =
            Scene { layers: scene.layers.clone(), templates: scene.templates.clone(), ..Scene::new([1, 1]) };
        (probe.assets, probe.effects) = (scene.assets.clone(), scene.effects.clone());
        let issue = match probe.read_files(Folder(dir)) {
            Err(SceneError::Invalid(issue)) => Some(issue),
            _ => None,
        };
        (probe.loaded, probe.models, issue)
    };
    let passing = |issue: &Issue| [STOPPED, UNCHECKED].iter().all(|t| !issue.to_string().contains(t));
    let kept = |read: &Read| read.2.as_ref().is_none_or(passing);
    let (loaded, models, issue) = remembered(&FILES, dir, key, make, kept);
    (scene.loaded, scene.models) = (loaded, models);
    issue
}
