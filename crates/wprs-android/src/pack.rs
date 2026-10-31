use crate::said;
use crate::{Error, store};
use image::{ImageReader, Limits, RgbaImage};
use std::collections::BTreeMap;
use std::fs;
use std::io::{BufReader, Cursor};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError, Weak};
use std::time::SystemTime;
use wprs_format::{Kind, Package};
use wprs_render::{Pictures, SceneAssets};
use wprs_scene::{Preset, Property, Scene};

mod info;

pub(crate) use info::{about, animated, check, entry, examples, lacks, media};

const IMAGES: [&str; 6] = ["png", "jpg", "jpeg", "webp", "gif", "bmp"];
const FONTS: [&str; 5] = ["ttf", "otf", "ttc", "otc", "woff2"];
const VIDEOS: [&str; 6] = ["webm", "mp4", "mkv", "mov", "m4v", "3gp"];
const SCENE_VIDEOS: [&str; 3] = ["webm", "mp4", "mkv"];
const DECODED: u64 = 256 << 20;
const SCENES: &str = "scenes";

type Stamp = (PathBuf, Option<SystemTime>, u64);
pub(crate) type Preview = (Option<RgbaImage>, Option<(u32, u32)>);

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
    pub(crate) scripts: Vec<(String, String)>,
    pub(crate) labels: serde_json::Value,
}

fn open(path: &Path) -> Result<Package<BufReader<fs::File>>, Error> {
    let package = wprs_format::open(path, &wprs_format::Limits::default())?;
    wprs_format::features::playable(&package.manifest, &package.facts, wprs_format::Platform::Android)?;
    match package.manifest.kind {
        kind if kind.first() => Ok(package),
        kind => Err(said("android.not_shown", "%1 packages are not wallpapers", &[&kind])),
    }
}

fn scene(path: &Path) -> Result<Package<BufReader<fs::File>>, Error> {
    let package = open(path)?;
    match package.manifest.kind {
        Kind::Scene => Ok(package),
        _ => Err(said(
            "android.scene_package",
            "%1 is not a scene package",
            &[&package.manifest.title_in(&[])],
        )),
    }
}

fn decode(bytes: Vec<u8>) -> Result<RgbaImage, Error> {
    let mut limits = Limits::default();
    (limits.max_image_width, limits.max_image_height, limits.max_alloc) =
        (Some(1 << 14), Some(1 << 14), Some(1 << 28));
    Ok(wprs_render::decode_limited(ImageReader::new(Cursor::new(bytes)), limits)?.into_rgba8())
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
    let mut package = scene(path)?;
    let mut scene = package.scene()?;
    if scene.layers().any(|layer| !layer.parallax.is_zero()) {
        scene.canvas.parallax.get_or_insert_with(Default::default).gyro = true;
    }
    let (mut assets, mut videos, mut decoded) = (SceneAssets::default(), Vec::new(), Pictures::default());
    for (id, asset) in &scene.assets {
        let kind = extension(&asset.path);
        if IMAGES.contains(&kind.as_str()) {
            let read = || Ok(package.read_file(&asset.path)?);
            if decoded.add::<Error>((&mut assets, id, asset), read, decode)? > DECODED {
                return Err(said(
                    "android.images_too_large",
                    "the images of %1 are too large",
                    &[&package.manifest.title_in(&[])],
                ));
            }
        } else if FONTS.contains(&kind.as_str()) {
            assets.fonts.insert(id.clone(), Arc::new(package.read_file(&asset.path)?));
        } else if SCENE_VIDEOS.contains(&kind.as_str()) {
            let file = folder(path)?.join(format!("{id}.{kind}"));
            if !file.is_file() {
                fs::create_dir_all(folder(path)?)?;
                store::write(&file, &package.read_file(&asset.path)?)?;
            }
            videos.push((id.clone(), file));
        }
    }
    assets.luts.extend(package.luts(&scene)?);
    let (file, kept) = (path.to_owned(), stamp.clone());
    assets.reloadable(&scene, move || {
        self::stamp(&file).ok().filter(|now| *now == kept)?;
        let mut package = self::scene(&file).ok()?;
        Some(move |path: &str| package.read_file(path).map_err(|e| e.to_string()))
    });
    let mut scripts = Vec::new();
    for script in &scene.scripts {
        let text = String::from_utf8(package.read_file(&script.path)?);
        let text = text.map_err(|_| said("android.utf_text", "%1 is not UTF-8 text", &[&script.path]))?;
        scripts.push((script.path.clone(), text));
    }
    if scripts.iter().map(|(_, s)| s.len()).sum::<usize>() > wprs_scene::MAX_SCRIPT_BYTES {
        return Err(said("android.scene_scripts_take", "the scene scripts take more than 1 MiB", &[]));
    }
    let (m, inputs) = (package.manifest.localized(), package.manifest.declared());
    let labels = wprs_scene::labels(&m.properties, &m.default_locale.iter().cloned().collect::<Vec<_>>());
    let (id, version, locale, scene) = (m.id, m.version, m.default_locale, Arc::new(scene));
    let (properties, presets) = (m.properties, m.presets);
    let pack = Arc::new(Pack {
        id,
        version,
        locale,
        scene,
        properties,
        presets,
        assets,
        videos,
        inputs,
        scripts,
        labels,
    });
    loaded.push((stamp, Arc::downgrade(&pack)));
    Ok(pack)
}
