use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

type Stamp = (i64, i64, u64);
type Cache = Mutex<BTreeMap<(PathBuf, [u32; 2]), (Stamp, Arc<Value>)>>;
static KNOWN: Cache = Mutex::new(BTreeMap::new());
static BODIES: Cache = Mutex::new(BTreeMap::new());

fn kept(cache: &Cache, path: PathBuf, size: [f32; 2], make: impl FnOnce() -> Value) -> Arc<Value> {
    let meta = std::fs::metadata(&path).ok();
    let stamp = meta.map_or((0, 0, 0), |m| (m.mtime(), m.mtime_nsec(), m.len()));
    let key = (path, size.map(f32::to_bits));
    if let Some((_, v)) =
        cache.lock().unwrap_or_else(|e| e.into_inner()).get(&key).filter(|(s, _)| *s == stamp)
    {
        return v.clone();
    }
    let found = Arc::new(make());
    let mut cache = cache.lock().unwrap_or_else(|e| e.into_inner());
    if cache.len() >= 32 {
        cache.clear();
    }
    cache.insert(key, (stamp, found.clone()));
    found
}

fn file(dir: &Path, doc: &Value, layer: &Value) -> Option<PathBuf> {
    let path = layer["mesh"].as_str().and_then(|m| crate::rig::asset(doc, m)).map(|p| dir.join(p));
    path.filter(|_| layer["type"] == "mesh")
}

pub(super) fn body(dir: &Path, doc: &Value, layer: &Value) -> Arc<Value> {
    match file(dir, doc, layer) {
        Some(path) => kept(&BODIES, path, crate::skin::size(doc, layer), || super::body(dir, doc, layer)),
        None => Arc::new(super::body(dir, doc, layer)),
    }
}

fn names(list: &Value) -> Vec<Value> {
    list.as_array().into_iter().flatten().map(|b| b.get("name").unwrap_or(b).clone()).collect()
}

pub fn known(dir: &Path, doc: &Value, id: &str) -> Arc<Value> {
    let Ok((_, layer)) = crate::actions::layer(doc, id) else { return Arc::new(Value::Null) };
    let Some(path) = file(dir, doc, layer) else { return Arc::new(Value::Null) };
    kept(&KNOWN, path, [0.0; 2], || {
        let rig = super::rig(dir, doc, id);
        json!({
            "bones": names(&rig["bones"]),
            "points": names(&rig["points"]),
            "parts": rig["parts"],
            "shapes": rig["shapes"],
        })
    })
}
