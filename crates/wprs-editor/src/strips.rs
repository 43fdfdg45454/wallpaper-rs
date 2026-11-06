use serde_json::{Value, json};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

const HEIGHT: u32 = 48;
const KEPT: usize = 512;

fn cache() -> Option<PathBuf> {
    Some(dirs::cache_dir()?.join("wallpaper-rs").join("strips"))
}

fn trim(dir: &Path) {
    let mut kept: Vec<_> = std::fs::read_dir(dir).into_iter().flatten().flatten().collect();
    if kept.len() <= KEPT {
        return;
    }
    kept.sort_by_key(|e| e.metadata().and_then(|m| m.modified()).ok());
    kept.iter().take(kept.len() - KEPT / 2).for_each(|e| drop(std::fs::remove_file(e.path())));
}

fn whole(found: &Value) -> bool {
    found
        .as_array()
        .is_some_and(|l| l.iter().all(|f| f["file"].as_str().is_some_and(|p| Path::new(p).is_file())))
}

pub fn strip(file: &Path, count: usize) -> Value {
    let count = count.clamp(1, wprs_video::MAX_STRIP);
    let (Some(dir), Ok(meta)) = (cache(), std::fs::metadata(file)) else { return json!([]) };
    let mut h = DefaultHasher::new();
    (file, meta.len(), meta.mtime(), meta.mtime_nsec(), count, HEIGHT).hash(&mut h);
    let key = format!("{:016x}", h.finish());
    let listed = dir.join(format!("{key}.json"));
    let kept = std::fs::read(&listed).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok());
    if let Some(found) = kept.filter(whole) {
        return found;
    }
    let Ok(frames) = wprs_video::strip(file, count, HEIGHT) else { return json!([]) };
    if std::fs::create_dir_all(&dir).is_err() {
        return json!([]);
    }
    trim(&dir);
    let saved = frames.into_iter().enumerate().filter_map(|(i, (at, still))| {
        let path = dir.join(format!("{key}-{i}.png"));
        image::RgbaImage::from_raw(still.width, still.height, still.rgba)?.save(&path).ok()?;
        Some(json!({ "at": at, "file": path }))
    });
    let out = Value::Array(saved.collect());
    drop(std::fs::write(&listed, out.to_string()));
    out
}
