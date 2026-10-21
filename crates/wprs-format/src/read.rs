use crate::validate::within;
use crate::{Error, FileEntry, Limits, MANIFEST, Manifest, Result, check_path, ensure, validate};
use std::fs::{self, File, FileType, OpenOptions};
use std::io::{self, BufReader, Read, Seek, Write};
use std::path::Path;
use zip::{CompressionMethod, ZipArchive};

const MAX_MANIFEST: u64 = 1 << 20;

#[derive(Debug)]
pub struct Package<R> {
    pub manifest: Manifest,
    pub entries: Vec<FileEntry>,
    archive: ZipArchive<R>,
}

pub fn open(path: impl AsRef<Path>, limits: &Limits) -> Result<Package<BufReader<File>>> {
    read(BufReader::new(File::open(path)?), limits)
}

pub fn read<R: Read + Seek>(reader: R, limits: &Limits) -> Result<Package<R>> {
    let mut archive = ZipArchive::new(reader)?;
    within("entry count", archive.len() as u64, limits.max_entries as u64)?;
    archive.validate_header_consistency()?;
    let mut entries = Vec::with_capacity(archive.len());
    for index in 0..archive.len() {
        let file = archive.by_index_data(index)?;
        let path = file.name()?.into_owned();
        ensure(!file.encrypted(), || Error::Unsupported(path.clone(), "encryption"))?;
        ensure(!file.is_symlink(), || Error::Symlink(path.clone()))?;
        let method = matches!(file.compression(), CompressionMethod::Stored | CompressionMethod::Deflated);
        ensure(method, || Error::Unsupported(path.clone(), "compression method"))?;
        if file.is_dir() {
            check_path(path.trim_end_matches('/'))?;
            continue;
        }
        let (size, compressed_size) = (file.size(), file.compressed_size());
        entries.push(FileEntry { path, size, compressed_size, symlink: false });
    }
    ensure(entries.iter().any(|e| e.path == MANIFEST), || Error::Missing(MANIFEST.into()))?;
    let mut bytes = Vec::new();
    archive.by_name(MANIFEST)?.take(MAX_MANIFEST + 1).read_to_end(&mut bytes)?;
    within("manifest size", bytes.len() as u64, MAX_MANIFEST)?;
    let manifest = Manifest::from_json(&bytes)?;
    validate(&manifest, &entries, limits)?;
    Ok(Package { manifest, entries, archive })
}

impl<R: Read + Seek> Package<R> {
    pub fn read_file(&mut self, path: &str) -> Result<Vec<u8>> {
        let index =
            self.entries.iter().position(|e| e.path == path).ok_or_else(|| Error::Missing(path.into()))?;
        let mut data = Vec::new();
        self.copy(index, &mut data)?;
        Ok(data)
    }

    fn copy(&mut self, index: usize, out: &mut impl Write) -> Result<()> {
        let FileEntry { path, size, .. } = &self.entries[index];
        let mut file = self.archive.by_name(path)?;
        let copied = io::copy(&mut file.by_ref().take(size.saturating_add(1)), out)?;
        ensure(copied == *size, || Error::SizeMismatch(path.clone()))
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
