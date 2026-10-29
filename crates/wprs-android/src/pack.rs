use crate::{Error, store};
use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageReader, Limits, RgbaImage};
use std::collections::BTreeMap;
use std::fs;
use std::io::{BufReader, Cursor};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError, Weak};
use std::time::SystemTime;
use wprs_format::{Kind, Package};
use wprs_render::SceneAssets;
use wprs_scene::{Preset, Property, Scene};

const IMAGES: [&str; 6] = ["png", "jpg", "jpeg", "webp", "gif", "bmp"];
const FONTS: [&str; 2] = ["ttf", "otf"];
const VIDEOS: [&str; 3] = ["webm", "mp4", "mkv"];
const DECODED: u64 = 256 << 20;
const SCENES: &str = "scenes";

type Stamp = (PathBuf, Option<SystemTime>, u64);
type Preview = (Option<RgbaImage>, Option<(u32, u32)>);

static LOADED: Mutex<Vec<(Stamp, Weak<Pack>)>> = Mutex::new(Vec::new());

pub(crate) struct Pack {
    pub(crate) id: String,
    pub(crate) version: String,
    pub(crate) locale: Option<String>,
    pub(crate) scene: Arc<Scene>,
    pub(crate) properties: BTreeMap<String, Property>,
    pub(crate) presets: Vec<Preset>,
    pub(crate) assets: SceneAssets,
    pub(crate) videos: Vec<(String, PathBuf)>,
    pub(crate) inputs: Option<wprs_scene::Uses>,
}

fn open(path: &Path) -> Result<Package<BufReader<fs::File>>, Error> {
    let package = wprs_format::open(path, &wprs_format::Limits::default())?;
    wprs_format::features::playable(&package.manifest, &package.facts, wprs_format::Platform::Android)?;
    match package.manifest.kind {
        Kind::Scene => Ok(package),
        _ => Err(Error::Scene(format!("{} is not a scene package", package.manifest.title_in(&[])))),
    }
}

fn decode(bytes: Vec<u8>) -> Result<RgbaImage, Error> {
    let mut reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = Limits::default();
    (limits.max_image_width, limits.max_image_height, limits.max_alloc) =
        (Some(1 << 14), Some(1 << 14), Some(1 << 28));
    reader.limits(limits);
    let mut decoder = reader.into_decoder()?;
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut image = DynamicImage::from_decoder(decoder)?;
    image.apply_orientation(orientation);
    Ok(image.into_rgba8())
}

pub(crate) fn check(path: &Path) -> Result<Preview, Error> {
    let mut package = open(path)?;
    let [width, height] = package.scene()?.canvas.size;
    let preview = package.manifest.thumbnail().cloned().map(|p| package.read_file(&p)).transpose()?;
    Ok((preview.and_then(|bytes| decode(bytes).ok()), Some((width, height))))
}

pub(crate) fn about(path: &Path, languages: &[String]) -> Option<wprs_core::model::About> {
    let mut about = wprs_format::peek(path).ok()?.about(languages);
    if about.schema != wprs_format::SCHEMA_V1 {
        about.description = wprs_core::markup::plain(&about.description);
        about.changes = wprs_core::markup::plain(&about.changes);
    }
    Some(about)
}

pub(crate) fn lacks(path: &Path, facets: &[String]) -> Vec<String> {
    let listed = wprs_format::peek(path).map(|m| m.features().to_vec()).unwrap_or_default();
    let mut found = wprs_format::features::lacks("android", listed.iter().chain(facets));
    found.sort();
    found.dedup();
    found
}

pub(crate) fn folder(path: &Path) -> Result<PathBuf, Error> {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
    Ok(store::dir()?.join(SCENES).join(stem))
}

fn extension(path: &str) -> String {
    Path::new(path).extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase()
}

fn stamp(path: &Path) -> Result<Stamp, Error> {
    let meta = fs::metadata(path)?;
    Ok((path.to_owned(), meta.modified().ok(), meta.len()))
}

pub(crate) fn load(path: &Path) -> Result<Arc<Pack>, Error> {
    let stamp = stamp(path)?;
    let mut loaded = LOADED.lock().unwrap_or_else(PoisonError::into_inner);
    loaded.retain(|(_, weak)| weak.strong_count() > 0);
    if let Some(pack) = loaded.iter().find(|(s, _)| *s == stamp).and_then(|(_, weak)| weak.upgrade()) {
        return Ok(pack);
    }
    let mut package = open(path)?;
    let scene = package.scene()?;
    let (mut assets, mut videos, mut decoded) = (SceneAssets::default(), Vec::new(), BTreeMap::new());
    for (id, asset) in &scene.assets {
        let kind = extension(&asset.path);
        if IMAGES.contains(&kind.as_str()) {
            let picture = match decoded.get(&asset.path) {
                Some(picture) => Arc::clone(picture),
                None => Arc::new(decode(package.read_file(&asset.path)?)?),
            };
            decoded.insert(asset.path.clone(), Arc::clone(&picture));
            let bytes = |i: &Arc<RgbaImage>| u64::from(i.width()) * u64::from(i.height()) * 4;
            if decoded.values().map(bytes).sum::<u64>() > DECODED {
                return Err(Error::Scene(format!(
                    "the images of {} are too large",
                    package.manifest.title_in(&[])
                )));
            }
            assets.images.insert(id.clone(), picture);
        } else if FONTS.contains(&kind.as_str()) {
            assets.fonts.insert(id.clone(), Arc::new(package.read_file(&asset.path)?));
        } else if VIDEOS.contains(&kind.as_str()) {
            let file = folder(path)?.join(format!("{id}.{kind}"));
            if !file.is_file() {
                fs::create_dir_all(folder(path)?)?;
                store::write(&file, &package.read_file(&asset.path)?)?;
            }
            videos.push((id.clone(), file));
        }
    }
    assets.luts.extend(package.luts(&scene)?);
    let (m, inputs) = (package.manifest.localized(), package.manifest.declared());
    let (id, version, locale, scene) = (m.id, m.version, m.default_locale, Arc::new(scene));
    let (properties, presets) = (m.properties, m.presets);
    let pack = Arc::new(Pack { id, version, locale, scene, properties, presets, assets, videos, inputs });
    loaded.push((stamp, Arc::downgrade(&pack)));
    Ok(pack)
}

pub(crate) fn examples(dir: &Path) -> Result<Vec<String>, Error> {
    let mut sources: Vec<PathBuf> = fs::read_dir(dir)?.flatten().map(|e| e.path()).collect();
    sources.retain(|source| source.join(wprs_format::MANIFEST).is_file());
    sources.sort();
    let built = tempfile::tempdir_in(store::dir()?)?;
    let mut added = Vec::new();
    for source in sources {
        let name = source.file_name().and_then(|n| n.to_str()).unwrap_or("scene").to_owned();
        let wpak = built.path().join(format!("{name}.wpak"));
        wprs_format::build(&source, &wpak, &wprs_format::Limits::default())?;
        added.push(store::store(&mut fs::File::open(&wpak)?, &name)?.0);
    }
    Ok(added)
}
