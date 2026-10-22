use super::Edit;
use crate::store::write_private;
use sha2::{Digest, Sha256};
use std::io::ErrorKind::NotFound;
use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub const MAX_BYTES: usize = 2 << 20;

pub fn dir() -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("wallpaper-rs").join("edits")
}

fn places(image: &Path, edits: &Path) -> [PathBuf; 2] {
    let canonical = std::fs::canonicalize(image).or_else(|_| std::path::absolute(image));
    let key = Sha256::digest(canonical.unwrap_or_else(|_| image.into()).as_os_str().as_bytes());
    let hex: String = key.iter().map(|b| format!("{b:02x}")).collect();
    [image.with_added_extension("wprs.json"), edits.join(hex + ".json")]
}

fn read(path: &Path) -> Result<Option<(SystemTime, String)>, String> {
    let open = || -> std::io::Result<_> {
        let file = std::fs::File::open(path)?;
        let (modified, mut text) = (file.metadata()?.modified()?, String::new());
        file.take(MAX_BYTES as u64 + 1).read_to_string(&mut text)?;
        Ok((modified, text))
    };
    match open() {
        Ok((_, text)) if text.len() > MAX_BYTES => Err(format!("{} is too large", path.display())),
        Ok(found) => Ok(Some(found)),
        Err(e) if e.kind() == NotFound => Ok(None),
        Err(e) => Err(format!("cannot read {}: {e}", path.display())),
    }
}

pub fn load(image: &Path, edits: &Path) -> Result<Option<(PathBuf, Edit)>, String> {
    let mut newest: Option<(SystemTime, PathBuf, String)> = None;
    for path in places(image, edits) {
        if let Some((time, text)) = read(&path)?
            && newest.as_ref().is_none_or(|(t, ..)| time > *t)
        {
            newest = Some((time, path, text));
        }
    }
    let Some((_, path, text)) = newest else { return Ok(None) };
    let edit = Edit::parse(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(Some((path, edit)))
}

pub fn save(edit: &Edit, image: &Path, edits: &Path) -> Result<PathBuf, String> {
    edit.validate()?;
    let (text, [beside, fallback]) = (edit.to_json(), places(image, edits));
    if !beside.is_symlink() && write_private(&beside, text.as_bytes()).is_ok() {
        std::fs::remove_file(&fallback).ok();
        return Ok(beside);
    }
    write_private(&fallback, text.as_bytes()).map(|()| fallback).map_err(|e| e.to_string())
}

pub fn remove(image: &Path, edits: &Path) -> Result<bool, String> {
    places(image, edits).iter().try_fold(false, |found, path| match std::fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == NotFound => Ok(found),
        Err(e) => Err(format!("cannot remove {}: {e}", path.display())),
    })
}
