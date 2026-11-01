use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

type Stamp = (i64, i64, u64);
static KNOWN: Mutex<BTreeMap<PathBuf, (Stamp, Arc<Value>)>> = Mutex::new(BTreeMap::new());

fn names(list: &Value) -> Vec<Value> {
    list.as_array().into_iter().flatten().map(|b| b.get("name").unwrap_or(b).clone()).collect()
}

pub fn known(dir: &Path, doc: &Value, id: &str) -> Arc<Value> {
    let Ok((_, layer)) = crate::actions::layer(doc, id) else { return Arc::new(Value::Null) };
    let path = layer["mesh"].as_str().and_then(|m| crate::rig::asset(doc, m)).map(|p| dir.join(p));
    let Some(path) = path.filter(|_| layer["type"] == "mesh") else { return Arc::new(Value::Null) };
    let meta = std::fs::metadata(&path).ok();
    let stamp = meta.map_or((0, 0, 0), |m| (m.mtime(), m.mtime_nsec(), m.len()));
    let mut cache = KNOWN.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, v)) = cache.get(&path).filter(|(s, _)| *s == stamp) {
        return v.clone();
    }
    let rig = super::rig(dir, doc, id);
    let found = Arc::new(json!({
        "bones": names(&rig["bones"]),
        "points": names(&rig["points"]),
        "parts": rig["parts"],
        "shapes": rig["shapes"],
    }));
    if cache.len() >= 32 {
        cache.clear();
    }
    cache.insert(path, (stamp, found.clone()));
    found
}
