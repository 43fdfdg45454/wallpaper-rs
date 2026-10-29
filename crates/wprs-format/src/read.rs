use crate::Facts;
use crate::contents::{binaries, preset_files, sounds};
use crate::features::{self, facts};
use crate::locale::{MAX_LOCALE_FILE, locale_file};
use crate::validate::{extension, within};
use crate::{
    Error, FileEntry, Kind, Limits, MANIFEST, Manifest, PresetFile, Result, check_path, ensure, media,
    validate,
};
use std::collections::BTreeMap;
use std::fs::{self, File, FileType, OpenOptions};
use std::io::{self, BufReader, Read, Seek, Write};
use std::path::Path;
use std::sync::Arc;
use wprs_scene::{Lut, MAX_LUT_BYTES, MAX_LUT_TOTAL, Scene};
use zip::{CompressionMethod, ZipArchive};

const MAX_MANIFEST: u64 = 1 << 20;
const MAX_SCENE: u64 = 8 << 20;
const MAX_THUMBNAIL: u64 = 16 << 20;
const MAX_PRESET: u64 = 256 << 10;

#[derive(Debug)]
pub struct Package<R> {
    pub manifest: Manifest,
    pub entries: Vec<FileEntry>,
    pub facts: Facts,
    videos: Vec<String>,
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
    let manifest = strings(&mut archive, Manifest::from_json(&raw)?)?;
    validate(&manifest, &entries, limits)?;
    let mut package = Package { manifest, entries, facts: Facts::default(), videos: Vec::new(), archive };
    let scene = matches!(package.manifest.kind, Kind::Scene | Kind::Assets)
        .then(|| package.scene_text())
        .transpose()?;
    let (scene, text) = scene.unzip();
    let size = match package.manifest.kind {
        Kind::Image => package.head(&package.manifest.entry.clone(), 1 << 16)?,
        Kind::Preset => package.preset().map(|_| Vec::new())?,
        Kind::Web => {
            let entries = package.entries.clone();
            binaries(&entries, |path| package.head(path, 4)).map(|()| Vec::new())?
        }
        _ => Vec::new(),
    };
    let size = media::probe(&size).map(|m| [m.width, m.height]);
    package.facts = facts(&package.manifest, scene.as_ref(), size);
    let newer = Manifest::newer_names(&raw, text.as_deref(), &package.facts.features);
    package.facts.features.extend(newer);
    package.videos = media::videos(&package.manifest, scene.as_ref());
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
    let manifest = Manifest::from_json(&raw(archive, MANIFEST, MAX_MANIFEST)?)?;
    strings(archive, manifest)
}

fn strings<R: Read + Seek>(archive: &mut ZipArchive<R>, mut manifest: Manifest) -> Result<Manifest> {
    let names = archive.file_names().filter_map(|n| n.ok().filter(|n| locale_file(n)).map(String::from));
    let names: Vec<String> = names.collect();
    manifest.read_strings(names, |name| {
        within("file size", archive.by_name(name)?.size(), MAX_LOCALE_FILE)?;
        raw(archive, name, MAX_LOCALE_FILE)
    })?;
    Ok(manifest)
}

impl<R: Read + Seek> Package<R> {
    pub fn scene(&mut self) -> Result<Scene> {
        self.scene_text().map(|(scene, _)| scene)
    }

    fn scene_text(&mut self) -> Result<(Scene, String)> {
        let entry = self.manifest.entry.clone();
        let size = self.entries.iter().find(|e| e.path == entry).map_or(0, |e| e.size);
        within("scene size", size, MAX_SCENE)?;
        let text = String::from_utf8(self.read_file(&entry)?)
            .map_err(|_| invalid("scene.json is not UTF-8".into()))?;
        let mut scene = Scene::load(&text, &self.manifest.properties).map_err(|e| invalid(e.to_string()))?;
        let missing =
            scene.paths().into_iter().find(|p| !self.entries.iter().any(|e| e.path == *p)).map(String::from);
        missing.map_or(Ok(()), |path| Err(Error::Missing(path)))?;
        let entries = self.entries.clone();
        sounds(&scene, &entries, |path, edge| match edge {
            Some(edge) => self.ends(path, edge).and_then(|(head, tail)| match tail {
                Some(tail) => Ok((head, tail)),
                None => Ok((self.read_file(path)?, Vec::new())),
            }),
            None => Ok((self.read_file(path)?, Vec::new())),
        })?;
        let props = self.manifest.properties.clone();
        scene.adopt(&props, Entries(self)).map_err(|e| invalid(e.to_string()))?;
        Ok((scene, text))
    }

    pub fn preset(&mut self) -> Result<PresetFile> {
        let entry = self.manifest.entry.clone();
        let size = self.entries.iter().find(|e| e.path == entry).map_or(0, |e| e.size);
        within("preset size", size, MAX_PRESET)?;
        let preset = PresetFile::parse(&self.read_file(&entry)?, &self.manifest)?;
        preset_files(&preset, &self.entries).map(|()| preset)
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

    pub fn look_at_videos(&mut self) -> Result<()> {
        for path in self.videos.clone() {
            let (head, tail) = self.ends(&path, media::EDGE)?;
            if media::hdr(&head) || tail.is_some_and(|t| media::hdr(&t)) {
                media::mark(&mut self.facts, &path);
            }
        }
        Ok(())
    }

    fn ends(&mut self, path: &str, edge: u64) -> Result<(Vec<u8>, Option<Vec<u8>>)> {
        let head = self.head(path, edge)?;
        let Ok(mut file) = self.archive.by_name_seek(path) else { return Ok((head, None)) };
        let size = file.seek(io::SeekFrom::End(0))?;
        file.seek(io::SeekFrom::Start(size.saturating_sub(edge).max(edge)))?;
        let mut tail = Vec::new();
        file.read_to_end(&mut tail)?;
        Ok((head, Some(tail)))
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

struct Entries<'a, R>(&'a mut Package<R>);

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

fn invalid(reason: String) -> Error {
    Error::Invalid { field: "scene".into(), reason }
}
