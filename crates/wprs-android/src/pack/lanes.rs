use super::Pack;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn lanes(pack: &Pack, id: &str, file: &Path) -> Vec<(String, PathBuf)> {
    let Ok(dir) = crate::store::dir().map(|d| d.join("lanes").join(&pack.id)) else { return Vec::new() };
    let kind = file.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default();
    let linked = |key: String| {
        fs::create_dir_all(&dir).ok()?;
        let link = dir.join(format!("{key}.{kind}"));
        if fs::read_link(&link).ok().as_deref() != Some(file) {
            drop(fs::remove_file(&link));
            std::os::unix::fs::symlink(file, &link).ok()?;
        }
        Some((key, link))
    };
    let own = pack.scene.lanes().into_iter().filter(|(_, asset)| asset == id);
    own.filter_map(|(key, _)| linked(key)).collect()
}
