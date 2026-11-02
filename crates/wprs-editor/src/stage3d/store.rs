use anyhow::{Result, anyhow};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::SystemTime;
use wprs_core::message::said;
use wprs_scene::{MAX_MODEL_BYTES, Model, ModelFile};

type Read = (Arc<Model>, Arc<ModelFile>);
type Kept = HashMap<PathBuf, (Option<SystemTime>, u64, Read)>;

static KEPT: Mutex<Option<Kept>> = Mutex::new(None);

pub fn file(path: &Path) -> Result<Read> {
    let meta = std::fs::metadata(path)?;
    let stamp = (meta.modified().ok(), meta.len());
    let mut kept = KEPT.lock().unwrap_or_else(PoisonError::into_inner);
    let kept = kept.get_or_insert_with(HashMap::new);
    if let Some((m, l, read)) = kept.get(path).filter(|(m, l, _)| (*m, *l) == stamp) {
        let _ = (m, l);
        return Ok(read.clone());
    }
    let bytes = std::fs::read(path)?;
    let tell = |e: String| anyhow!(said("editor.model_file", "%1 %2", &[&path.display(), &e]));
    if bytes.len() > MAX_MODEL_BYTES {
        return Err(tell(format!("is larger than {} MiB", MAX_MODEL_BYTES >> 20)));
    }
    let head = Arc::new(ModelFile::parse(&bytes).map_err(tell)?);
    let read = (Arc::new(Model::decode(&bytes).map_err(tell)?), head);
    kept.retain(|_, (_, _, r)| Arc::strong_count(&r.0) > 1);
    kept.insert(path.to_path_buf(), (stamp.0, stamp.1, read.clone()));
    Ok(read)
}

pub fn path(dir: &Path, doc: &Value, asset: &str) -> Result<PathBuf> {
    let found =
        doc.pointer(&format!("/scene/assets/{}/path", crate::ops::escape(asset))).and_then(Value::as_str);
    let found = found.filter(|p| p.to_ascii_lowercase().ends_with(".glb"));
    let p = found
        .ok_or_else(|| anyhow!(said("editor.not_model", "%1 is not a model of the scene", &[&asset])))?;
    Ok(dir.join(p))
}

pub fn asset(dir: &Path, doc: &Value, asset: &str) -> Result<Read> {
    file(&path(dir, doc, asset)?)
}

pub fn scene(dir: &Path, doc: &Value) -> Option<wprs_scene::Scene> {
    let mut scene = wprs_scene::Scene::from_json(&doc["scene"].to_string()).ok()?;
    let ids: Vec<String> = scene.layers().filter_map(|l| l.figure().map(String::from)).collect();
    for id in ids {
        if let Ok((model, head)) = asset(dir, doc, &id) {
            scene.solids.insert(id.clone(), model);
            scene.models.insert(id, head);
        }
    }
    Some(scene)
}
