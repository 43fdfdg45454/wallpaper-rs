use crate::locale::{MAX_LOCALE_FILE, locale_file};
use crate::validate::within;
use crate::{Error, MANIFEST, Manifest, Package, Result, check_path, ensure};
use std::fs::{self, File, FileType, OpenOptions};
use std::io::{self, BufReader, Read, Seek};
use std::path::Path;
use zip::ZipArchive;

pub(crate) const MAX_MANIFEST: u64 = 1 << 20;
pub(crate) const MAX_THUMBNAIL: u64 = 16 << 20;

pub fn peek(path: impl AsRef<Path>) -> Result<Manifest> {
    manifest(&mut ZipArchive::new(BufReader::new(File::open(path)?))?)
}

pub fn thumbnail(path: impl AsRef<Path>) -> Result<Option<Vec<u8>>> {
    let mut archive = ZipArchive::new(BufReader::new(File::open(path)?))?;
    let value: serde_json::Value = serde_json::from_slice(&raw(&mut archive, MANIFEST, MAX_MANIFEST)?)?;
    let name = value.pointer("/previews/thumbnail").or_else(|| value.get("preview")).and_then(|v| v.as_str());
    let Some(name) = name else { return Ok(None) };
    check_path(name)?;
    raw(&mut archive, name, MAX_THUMBNAIL).map(Some)
}

pub(crate) fn raw<R: Read + Seek>(archive: &mut ZipArchive<R>, name: &str, max: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    archive.by_name(name)?.take(max + 1).read_to_end(&mut bytes)?;
    within("file size", bytes.len() as u64, max)?;
    Ok(bytes)
}

fn manifest<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<Manifest> {
    let manifest = Manifest::from_json(&raw(archive, MANIFEST, MAX_MANIFEST)?)?;
    strings(archive, manifest)
}

pub(crate) fn strings<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    mut manifest: Manifest,
) -> Result<Manifest> {
    let names = archive.file_names().filter_map(|n| n.ok().filter(|n| locale_file(n)).map(String::from));
    let names: Vec<String> = names.collect();
    manifest.read_strings(names, |name| {
        within("file size", archive.by_name(name)?.size(), MAX_LOCALE_FILE)?;
        raw(archive, name, MAX_LOCALE_FILE)
    })?;
    Ok(manifest)
}

pub(crate) struct Entries<'a, R>(pub(crate) &'a mut Package<R>);

impl<R: Read + Seek> wprs_scene::Files for Entries<'_, R> {
    type Error = Error;

    fn read(&mut self, path: &str, max: usize) -> Result<Vec<u8>> {
        self.head(path, max, usize::MAX).map(|(bytes, _)| bytes)
    }

    fn head(&mut self, path: &str, max: usize, keep: usize) -> Result<(Vec<u8>, u64)> {
        let size = self.0.entries.iter().find(|e| e.path == path).map_or(0, |e| e.size);
        within("scene file size", size, max as u64)?;
        let whole = keep as u64 >= size;
        Ok((if whole { self.0.read_file(path)? } else { self.0.head(path, keep as u64)? }, size))
    }
}

pub fn extract<R: Read + Seek>(package: &mut Package<R>, dest: impl AsRef<Path>) -> Result<()> {
    let dest = dest.as_ref();
    fs::create_dir_all(dest)?;
    ensure(!fs::symlink_metadata(dest)?.is_symlink(), || Error::Symlink(dest.display().to_string()))?;
    (0..package.entries.len()).try_for_each(|index| {
        let mut file = create(dest, &package.entries[index].path)?;
        package.copy(index, &mut file)
    })
}

fn create(dest: &Path, path: &str) -> Result<File> {
    check_path(path)?;
    let mut target = dest.to_path_buf();
    let segments: Vec<&str> = path.split('/').collect();
    for (index, segment) in segments.iter().enumerate() {
        target.push(segment);
        let last = index + 1 == segments.len();
        match file_type(&target)? {
            Some(kind) if kind.is_symlink() => return Err(Error::Symlink(target.display().to_string())),
            Some(kind) if kind.is_dir() && !last => {}
            Some(kind) if kind.is_file() && last => fs::remove_file(&target)?,
            Some(_) => return Err(Error::Blocked(target)),
            None if !last => fs::create_dir(&target)?,
            None => {}
        }
    }
    Ok(OpenOptions::new().write(true).create_new(true).open(&target)?)
}

fn file_type(path: &Path) -> Result<Option<FileType>> {
    match fs::symlink_metadata(path) {
        Ok(meta) => Ok(Some(meta.file_type())),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}
