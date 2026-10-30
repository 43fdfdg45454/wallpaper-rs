use crate::Facts;
use crate::build::collect;
use crate::contents::{Ends, binaries, preset_files, sounds};
use crate::features::facts;
use crate::locale::{LOCALE, MAX_LOCALE_FILE};
use crate::{Error, Kind, Limits, MANIFEST, Manifest, Result, SIGNATURE, Scene, media, require, validate};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use wprs_scene::Files;

pub fn inspect(source: impl AsRef<Path>) -> Result<(Manifest, Facts)> {
    let source = source.as_ref();
    let raw = fs::read(source.join(MANIFEST))?;
    let manifest = with_strings(source, Manifest::from_json(&raw)?)?;
    let facts = facts_of(source, &manifest, &raw, None)?;
    Ok((manifest, facts))
}

pub fn locale_names(source: &Path) -> Vec<String> {
    let files = fs::read_dir(source.join(LOCALE)).into_iter().flatten().flatten();
    let mut names: Vec<String> =
        files.map(|f| format!("{LOCALE}{}", f.file_name().to_string_lossy())).collect();
    names.sort();
    names
}

pub fn with_strings(source: &Path, mut manifest: Manifest) -> Result<Manifest> {
    manifest.read_strings(locale_names(source), |name| {
        crate::validate::within("file size", fs::metadata(source.join(name))?.len(), MAX_LOCALE_FILE)?;
        Ok(fs::read(source.join(name))?)
    })?;
    Ok(manifest)
}

pub struct Folder<'a>(pub &'a Path);

impl wprs_scene::Files for Folder<'_> {
    type Error = Error;

    fn read(&mut self, path: &str, max: usize) -> Result<Vec<u8>> {
        self.head(path, max, usize::MAX).map(|(bytes, _)| bytes)
    }

    fn head(&mut self, path: &str, max: usize, keep: usize) -> Result<(Vec<u8>, u64)> {
        crate::check_path(path)?;
        let size = fs::metadata(self.0.join(path))?.len();
        crate::validate::within("scene file size", size, max as u64)?;
        let mut data = Vec::new();
        File::open(self.0.join(path))?.take(keep as u64).read_to_end(&mut data)?;
        Ok((data, size))
    }
}

pub(crate) fn facts_of(
    source: &Path,
    manifest: &Manifest,
    raw: &[u8],
    scene: Option<String>,
) -> Result<Facts> {
    let mut files = Vec::new();
    collect(source, "", &mut files)?;
    validate(manifest, &files, &Limits::default())?;
    let entry = source.join(&manifest.entry);
    let text = match (manifest.kind, scene) {
        (Kind::Scene | Kind::Assets, None) => Some(fs::read_to_string(&entry)?),
        (_, text) => text,
    };
    let scene = match (manifest.kind, &text) {
        (Kind::Scene | Kind::Assets, Some(text)) => Some(Scene::load(text, &manifest.properties)),
        (Kind::Preset, _) => {
            let preset = crate::PresetFile::parse(&fs::read(&entry)?, manifest)?;
            preset_files(&preset, &files).map(|()| None)?
        }
        (Kind::Web, _) => {
            binaries(&files, |path| Folder(source).head(path, usize::MAX, 4).map(|r| r.0)).map(|()| None)?
        }
        _ => None,
    };
    let invalid = |e: wprs_scene::SceneError| Error::Invalid { field: "scene".into(), reason: e.to_string() };
    let mut scene = scene.transpose().map_err(invalid)?;
    if let Some(scene) = scene.as_mut() {
        sounds(scene, &files, |path, edge| match edge {
            Some(edge) => ends(&source.join(path), edge),
            None => Ok((fs::read(source.join(path))?, Vec::new())),
        })?;
        scene.adopt(&manifest.properties, Folder(source)).map_err(invalid)?;
    }
    let mut head = Vec::new();
    if manifest.kind == Kind::Image {
        File::open(&entry)?.take(1 << 16).read_to_end(&mut head)?;
    }
    let size = media::probe(&head).map(|m| [m.width, m.height]);
    let mut found = facts(manifest, scene.as_ref(), size);
    let newer = Manifest::newer_names(raw, text.as_deref(), &found.features);
    found.features.extend(newer);
    found.notes.extend(scene.iter().flat_map(|s| uneven(source, s)));
    found.notes.extend(manifest.unread_texts(locale_names(source)));
    for path in media::videos(manifest, scene.as_ref()) {
        let (head, tail) = ends(&source.join(&path), media::EDGE)?;
        if media::hdr(&head) || media::hdr(&tail) {
            media::mark(&mut found, &path);
        }
    }
    Ok(found)
}

fn ends(path: &Path, edge: u64) -> Result<Ends> {
    let mut file = File::open(path)?;
    let (mut head, mut tail) = (Vec::new(), Vec::new());
    file.by_ref().take(edge).read_to_end(&mut head)?;
    let size = file.seek(SeekFrom::End(0))?;
    file.seek(SeekFrom::Start(size.saturating_sub(edge).max(edge)))?;
    file.read_to_end(&mut tail)?;
    Ok((head, tail))
}

fn uneven(source: &Path, scene: &Scene) -> Vec<String> {
    let ratio = |id: &str| {
        let mut head = Vec::new();
        File::open(source.join(&scene.assets.get(id)?.path))
            .ok()?
            .take(1 << 16)
            .read_to_end(&mut head)
            .ok()?;
        media::probe(&head).map(|m| f64::from(m.width) / f64::from(m.height.max(1)))
    };
    let varied = |(place, ids): (String, Vec<&str>)| {
        let ratios: Vec<f64> = ids.into_iter().filter_map(ratio).collect();
        let (low, high) = ratios.iter().fold((f64::MAX, 0.0f64), |(a, b), r| (a.min(*r), b.max(*r)));
        (ratios.len() > 1 && high > low * 1.01)
            .then(|| format!("the images of {place} do not share one proportion, so some will be cropped"))
    };
    wprs_scene::variants(scene).into_iter().filter_map(varied).collect()
}

pub fn fill(source: impl AsRef<Path>) -> Result<Vec<String>> {
    let source = source.as_ref();
    let (mut manifest, facts) = inspect(source)?;
    let listed = manifest.features();
    let added: Vec<String> = facts.features.into_iter().filter(|f| !listed.contains(f)).collect();
    if !manifest.v2() || added.is_empty() {
        return Ok(Vec::new());
    }
    let reason = format!("misses {added:?}; run pack sign again, which fills the list before signing");
    require(!source.join(SIGNATURE).exists(), "requires.features", &reason)?;
    let features = &mut manifest.requires.get_or_insert_default().features;
    features.extend(added.iter().cloned());
    features.sort();
    fs::write(source.join(MANIFEST), manifest.to_json()? + "\n")?;
    Ok(added)
}
