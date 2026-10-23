use crate::{Error, Kind, Manifest, Result, ensure, require};
use std::collections::HashSet;
use std::path::Path;

pub const MANIFEST: &str = "manifest.json";
pub const SIGNATURE: &str = "manifest.json.minisig";

const MAX_PATH: usize = 255;
const RATIO_FLOOR: u64 = 1 << 20;
const IMAGES: &[&str] = &["png", "jpg", "jpeg", "webp", "avif", "gif", "bmp", "ktx2"];
const VIDEOS: &[&str] = &["webm", "mp4", "mkv"];
const SCENES: &[&str] =
    &["json", "wgsl", "js", "gltf", "glb", "bin", "ogg", "opus", "flac", "mp3", "wav", "ttf", "otf", "cube"];
const COMMON: &[&str] = &["txt"];
const THUMBNAILS: &[&str] = &["png", "jpg", "jpeg", "webp"];
const ANIMATED: &[&str] = &["gif", "webp"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub max_entries: usize,
    pub max_file_size: u64,
    pub max_total_size: u64,
    pub max_ratio: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Limits { max_entries: 4096, max_file_size: 1 << 30, max_total_size: 2 << 30, max_ratio: 100 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    pub path: String,
    pub size: u64,
    pub compressed_size: u64,
    pub symlink: bool,
}

pub fn check_path(path: &str) -> Result<()> {
    let safe = path.len() <= MAX_PATH
        && !path.chars().any(|c| c.is_control() || c == '\\' || c == ':')
        && path.split('/').all(|segment| !matches!(segment, "" | "." | ".."));
    ensure(safe, || Error::UnsafePath(path.into()))
}

pub fn validate(manifest: &Manifest, files: &[FileEntry], limits: &Limits) -> Result<()> {
    manifest.check()?;
    within("entry count", files.len() as u64, limits.max_entries as u64)?;
    let mut seen = HashSet::new();
    let mut total = 0u64;
    for file in files {
        check_path(&file.path)?;
        ensure(!file.symlink, || Error::Symlink(file.path.clone()))?;
        ensure(seen.insert(file.path.to_lowercase()), || Error::Duplicate(file.path.clone()))?;
        let allowed = [MANIFEST, SIGNATURE].contains(&file.path.as_str())
            || extension(&file.path).is_some_and(|e| allows(manifest.kind, &e));
        ensure(allowed, || Error::Extension { path: file.path.clone(), kind: manifest.kind })?;
        within("file size", file.size, limits.max_file_size)?;
        total = total.saturating_add(file.size);
        within("total size", total, limits.max_total_size)?;
        if file.size > RATIO_FLOOR {
            within("compression ratio", file.size / file.compressed_size.max(1), limits.max_ratio)?;
        }
    }
    ensure(files.iter().any(|f| f.path == MANIFEST), || Error::Missing(MANIFEST.into()))?;
    let entry_types = match manifest.kind {
        Kind::Image => IMAGES,
        Kind::Video => VIDEOS,
        Kind::Scene => &["json"],
    };
    let (thumbs, limit) = if manifest.v2() { (THUMBNAILS, 1 << 20) } else { (IMAGES, u64::MAX) };
    let previews = manifest.previews.clone().unwrap_or_default();
    let referenced = [
        ("entry", Some(&manifest.entry), entry_types, u64::MAX),
        ("preview", manifest.preview.as_ref(), IMAGES, u64::MAX),
        ("previews.thumbnail", previews.thumbnail.as_ref(), thumbs, limit),
        ("previews.animated", previews.animated.as_ref(), ANIMATED, 8 << 20),
    ];
    referenced.into_iter().try_for_each(|(field, path, types, max)| {
        let Some(path) = path else { return Ok(()) };
        check_path(path)?;
        let file = files.iter().find(|f| &f.path == path).ok_or_else(|| Error::Missing(path.clone()))?;
        let usable = path != MANIFEST && extension(path).is_some_and(|e| types.contains(&e.as_str()));
        require(usable, field, &format!("{path:?} is not a usable {field} for {} packages", manifest.kind))?;
        require(
            file.size <= max,
            field,
            &format!("{path:?} has {} bytes, more than the {max} allowed", file.size),
        )
    })?;
    manifest.credits.iter().flat_map(|c| &c.paths).try_for_each(|path| {
        check_path(path.trim_end_matches('/'))?;
        let dir = path.ends_with('/');
        let found =
            files.iter().any(|f| if dir { f.path.starts_with(path.as_str()) } else { f.path == *path });
        require(found, "credits", &format!("{path:?} is not a file or folder of the package"))
    })
}

pub(crate) fn within(what: &'static str, actual: u64, limit: u64) -> Result<()> {
    ensure(actual <= limit, || Error::Limit { what, actual, limit })
}

pub(crate) fn extension(path: &str) -> Option<String> {
    Path::new(path).extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase)
}

fn allows(kind: Kind, extension: &str) -> bool {
    let groups: &[&[&str]] = match kind {
        Kind::Image => &[IMAGES, COMMON],
        Kind::Video => &[IMAGES, VIDEOS, COMMON],
        Kind::Scene => &[IMAGES, VIDEOS, SCENES, COMMON],
    };
    groups.iter().any(|group| group.contains(&extension))
}
