use crate::features::{self, Facts, facts};
use crate::validate::{extension, within};
use crate::{
    Error, FileEntry, Kind, Limits, MANIFEST, Manifest, Result, check_path, ensure, media, validate,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, FileType, OpenOptions};
use std::io::{self, BufReader, Read, Seek, Write};
use std::path::Path;
use std::sync::Arc;
use wprs_scene::{Clip, Lut, MAX_LUT_BYTES, MAX_LUT_TOTAL, MAX_SOUND_BYTES, MAX_SOUND_TOTAL, Scene};
use zip::{CompressionMethod, ZipArchive};

const MAX_MANIFEST: u64 = 1 << 20;
const MAX_SCENE: u64 = 8 << 20;
const MAX_THUMBNAIL: u64 = 16 << 20;

#[derive(Debug)]
pub struct Package<R> {
    pub manifest: Manifest,
    pub entries: Vec<FileEntry>,
    pub facts: Facts,
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
    let raw = raw(&mut archive, MANIFEST, MAX_MANIFEST)?;
    let manifest = Manifest::from_json(&raw)?;
    validate(&manifest, &entries, limits)?;
    let mut package = Package { manifest, entries, facts: Facts::default(), archive };
    let scene = (package.manifest.kind == Kind::Scene).then(|| package.scene()).transpose()?;
    let size = match package.manifest.kind {
        Kind::Image => package.head(&package.manifest.entry.clone(), 1 << 16)?,
        _ => Vec::new(),
    };
    let size = media::probe(&size).map(|m| [m.width, m.height]);
    package.facts = facts(&package.manifest, scene.as_ref(), size);
    package.facts.features.extend(Manifest::newer_spelling(&raw).map(|_| "properties2".to_owned()));
    if package.manifest.v2() {
        features::check(&package.manifest, &package.facts)?;
        package.previews()?;
    }
    Ok(package)
}

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

fn raw<R: Read + Seek>(archive: &mut ZipArchive<R>, name: &str, max: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    archive.by_name(name)?.take(max + 1).read_to_end(&mut bytes)?;
    within("file size", bytes.len() as u64, max)?;
    Ok(bytes)
}

fn manifest<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<Manifest> {
    Manifest::from_json(&raw(archive, MANIFEST, MAX_MANIFEST)?)
}

impl<R: Read + Seek> Package<R> {
    pub fn scene(&mut self) -> Result<Scene> {
        let entry = self.manifest.entry.clone();
        let size = self.entries.iter().find(|e| e.path == entry).map_or(0, |e| e.size);
        within("scene size", size, MAX_SCENE)?;
        let text = String::from_utf8(self.read_file(&entry)?)
            .map_err(|_| invalid("scene.json is not UTF-8".into()))?;
        let scene = Scene::load(&text, &self.manifest.properties).map_err(|e| invalid(e.to_string()))?;
        let missing =
            scene.paths().into_iter().find(|p| !self.entries.iter().any(|e| e.path == *p)).map(String::from);
        missing.map_or(Ok(()), |path| Err(Error::Missing(path)))?;
        self.sounds(&scene)?;
        Ok(scene)
    }

    fn sounds(&mut self, scene: &Scene) -> Result<()> {
        let paths: BTreeSet<&str> =
            scene.sounds().filter_map(|s| scene.assets.get(&s.sound)).map(|a| a.path.as_str()).collect();
        let mut total = 0.0;
        for path in paths {
            let size = self.entries.iter().find(|e| e.path == path).map_or(0, |e| e.size);
            within("sound size", size, MAX_SOUND_BYTES)?;
            let clip = Clip::probe(&self.read_file(path)?)
                .map_err(|reason| Error::Invalid { field: path.into(), reason })?;
            total += clip.seconds;
            within("length of the sounds in seconds", total.ceil() as u64, MAX_SOUND_TOTAL as u64)?;
        }
        Ok(())
    }

    pub fn luts(&mut self, scene: &Scene) -> Result<Vec<(String, Arc<Lut>)>> {
        let (mut parsed, mut out) = (BTreeMap::<String, Arc<Lut>>::new(), Vec::new());
        for (id, asset) in scene.assets.iter().filter(|(_, a)| extension(&a.path).as_deref() == Some("cube"))
        {
            if let Some(lut) = parsed.get(&asset.path) {
                out.push((id.clone(), lut.clone()));
                continue;
            }
            let size = self.entries.iter().find(|e| e.path == asset.path).map_or(0, |e| e.size);
            within("LUT size", size, MAX_LUT_BYTES as u64)?;
            let lut = Lut::parse(&self.read_file(&asset.path)?)
                .map_err(|e| Error::Invalid { field: asset.path.clone(), reason: e.to_string() })?;
            parsed.insert(asset.path.clone(), Arc::new(lut));
            within(
                "LUT data",
                parsed.values().map(|l| l.data.len() as u64 * 12).sum(),
                MAX_LUT_TOTAL as u64,
            )?;
            out.extend(parsed.get(&asset.path).map(|lut| (id.clone(), lut.clone())));
        }
        Ok(out)
    }

    fn previews(&mut self) -> Result<()> {
        let previews = self.manifest.previews.clone().unwrap_or_default();
        for (animated, path) in [(false, previews.thumbnail), (true, previews.animated)] {
            let Some(path) = path else { continue };
            media::preview(animated, &path, &self.read_file(&path)?)?;
        }
        Ok(())
    }

    fn head(&mut self, path: &str, max: u64) -> Result<Vec<u8>> {
        let mut data = Vec::new();
        self.archive.by_name(path)?.take(max).read_to_end(&mut data)?;
        Ok(data)
    }

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

fn invalid(reason: String) -> Error {
    Error::Invalid { field: "scene".into(), reason }
}
