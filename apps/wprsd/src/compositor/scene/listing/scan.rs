use super::Files;
use std::hash::{BuildHasher, Hasher};
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::sync::LazyLock;

static SALT: LazyLock<std::hash::RandomState> = LazyLock::new(std::hash::RandomState::new);

pub(super) type Wanted = Vec<(String, Option<(PathBuf, wprs_scene::Property)>)>;

fn opaque(parts: &[&[u8]]) -> String {
    let mut h = SALT.build_hasher();
    parts.iter().for_each(|p| h.write(p));
    format!("{:016x}", h.finish())
}

pub(super) fn scan(wanted: Wanted) -> (String, Files) {
    let (mut listing, mut files) = (serde_json::Map::new(), Vec::new());
    for (id, chosen) in wanted {
        let found = chosen.map(|(dir, prop)| crate::picks::files(&dir, &prop)).unwrap_or_default();
        let entries = found.into_iter().map(|path| {
            let stamp = std::fs::metadata(&path).map_or([0; 2], |m| [m.size() as i64, m.mtime()]);
            let raw = path.as_os_str().as_encoded_bytes();
            let token = opaque(&[id.as_bytes(), raw, &stamp[0].to_le_bytes(), &stamp[1].to_le_bytes()]);
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let kind = if wprs_video::is_video(&path) { "video" } else { "image" };
            let entry = serde_json::json!([token, opaque(&[id.as_bytes(), raw]), name, kind]);
            files.push((format!("wpak-user://{id}/{token}"), path));
            entry
        });
        let entries = serde_json::Value::Array(entries.collect());
        listing.insert(id, entries);
    }
    (serde_json::Value::Object(listing).to_string(), files)
}
