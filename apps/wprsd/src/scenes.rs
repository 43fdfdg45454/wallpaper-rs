use crate::daemon::Daemon;
use crate::decode::decode;
use crate::store::private_dir;
use image::{ImageReader, RgbaImage};
use std::collections::BTreeMap;
use std::io::{Cursor, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use wprs_render::SceneAssets;
use wprs_scene::{Control, Preset, Property, Scene, Values};
use wprs_video::Bank;

const IMAGES: [&str; 6] = ["png", "jpg", "jpeg", "webp", "gif", "bmp"];
const FONTS: [&str; 2] = ["ttf", "otf"];
const VIDEOS: [&str; 3] = ["webm", "mp4", "mkv"];
const DECODED: u64 = 512 << 20;

pub struct ScenePack {
    pub path: PathBuf,
    pub id: String,
    pub title: String,
    pub languages: Vec<String>,
    pub locale: String,
    pub scene: Arc<Scene>,
    pub properties: BTreeMap<String, Property>,
    pub presets: Vec<Preset>,
    pub assets: SceneAssets,
    pub videos: Vec<(String, PathBuf)>,
    pub sounds: Vec<(String, PathBuf)>,
    pub clips: Bank,
    pub scripts: Vec<(String, String)>,
}

impl ScenePack {
    pub fn values(&self, layers: &[&BTreeMap<String, toml::Value>]) -> Values {
        wprs_scene::effective(&self.properties, &json(layers).iter().collect::<Vec<_>>())
    }

    pub fn stale(&self, layers: &[&BTreeMap<String, toml::Value>]) -> Vec<String> {
        wprs_scene::stale(&self.properties, &json(layers).iter().collect::<Vec<_>>())
    }

    pub fn files(&self, values: &Values) -> Vec<String> {
        let files = self.properties.iter().filter(|(_, p)| matches!(p.control, Control::File { .. }));
        files.filter_map(|(id, _)| values.get(id)?.as_str().map(String::from)).collect()
    }
}

fn json(layers: &[&BTreeMap<String, toml::Value>]) -> Vec<Values> {
    let pairs = |l: &&BTreeMap<String, toml::Value>| {
        l.iter().filter_map(|(k, v)| Some((k.clone(), serde_json::to_value(v).ok()?))).collect()
    };
    layers.iter().map(pairs).collect()
}

fn kind(path: &str) -> String {
    Path::new(path).extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase()
}

pub fn image(bytes: Vec<u8>) -> anyhow::Result<RgbaImage> {
    decode(ImageReader::new(Cursor::new(bytes))).map(|i| i.into_rgba8()).map_err(|e| match e {
        crate::decode::Problem::Missing(e) | crate::decode::Problem::Decode(e) => anyhow::anyhow!(e),
    })
}

impl Daemon {
    pub fn scene_pack(&self, wpak: &Path) -> anyhow::Result<Arc<ScenePack>> {
        let meta = std::fs::metadata(wpak)?;
        let stamp = (meta.mtime(), meta.len());
        let cached =
            self.state().scenes.get(wpak).filter(|(s, _)| *s == stamp).and_then(|(_, weak)| weak.upgrade());
        if let Some(pack) = cached {
            return Ok(pack);
        }
        let mut package = wprs_format::open(wpak, &wprs_format::Limits::default())?;
        anyhow::ensure!(
            package.manifest.kind == wprs_format::Kind::Scene,
            "{} is not a scene package",
            wpak.display()
        );
        let scene = package.scene()?;
        let (mut assets, mut videos, mut sounds) = (SceneAssets::default(), Vec::new(), Vec::new());
        let mut decoded: BTreeMap<String, Arc<RgbaImage>> = BTreeMap::new();
        for (id, asset) in &scene.assets {
            let ext = kind(&asset.path);
            if IMAGES.contains(&ext.as_str()) {
                let picture = match decoded.get(&asset.path) {
                    Some(picture) => picture.clone(),
                    None => Arc::new(image(package.read_file(&asset.path)?)?),
                };
                decoded.insert(asset.path.clone(), picture.clone());
                let bytes = |i: &Arc<RgbaImage>| u64::from(i.width()) * u64::from(i.height()) * 4;
                let total: u64 = decoded.values().map(bytes).sum();
                anyhow::ensure!(
                    total <= DECODED,
                    "the images of {} are too large once decoded",
                    wpak.display()
                );
                assets.images.insert(id.clone(), picture);
            } else if FONTS.contains(&ext.as_str()) {
                assets.fonts.insert(id.clone(), Arc::new(package.read_file(&asset.path)?));
            } else if VIDEOS.contains(&ext.as_str()) || scene.sounds().any(|s| s.sound == *id) {
                let blob = self.blob(wpak, stamp.0, stamp.1).with_extension(format!("{id}.{ext}"));
                if !blob.is_file() {
                    let dir = blob.parent().unwrap_or(Path::new("."));
                    private_dir(dir)?;
                    let mut file = tempfile::Builder::new().prefix(".tmp-").tempfile_in(dir)?;
                    file.write_all(&package.read_file(&asset.path)?)?;
                    file.persist(&blob)?;
                }
                let media = if VIDEOS.contains(&ext.as_str()) { &mut videos } else { &mut sounds };
                media.push((id.clone(), blob));
            }
        }
        assets.luts.extend(package.luts(&scene)?);
        let mut scripts = Vec::new();
        for script in &scene.scripts {
            let source = String::from_utf8(package.read_file(&script.path)?)
                .map_err(|_| anyhow::anyhow!("{} is not UTF-8 text", script.path))?;
            scripts.push((script.path.clone(), source));
        }
        let m = package.manifest.localized();
        let locale = m.locale_in(&wprs_core::locale::languages());
        let languages = m.languages(&wprs_core::locale::languages());
        let properties = scene.properties(&m.properties, &crate::words::volume());
        let clips = Bank::new(sounds.iter().map(|(_, path)| path.clone()).collect());
        let pack = Arc::new(ScenePack {
            path: wpak.to_owned(),
            id: m.id.clone(),
            title: m.title.pick(&languages),
            languages,
            locale,
            scene: Arc::new(scene),
            properties,
            presets: m.presets.clone(),
            assets,
            videos,
            sounds,
            clips,
            scripts,
        });
        let mut st = self.state();
        st.scenes.retain(|_, (_, weak)| weak.strong_count() > 0);
        st.scenes.insert(wpak.to_owned(), (stamp, Arc::downgrade(&pack)));
        Ok(pack)
    }

    pub fn scene_file(&self, file: &str) -> anyhow::Result<Arc<RgbaImage>> {
        let path = Path::new(file);
        anyhow::ensure!(path.is_file(), "it is not a file");
        self.load_image(path, std::fs::metadata(path)?.modified().ok(), None)
    }

    pub fn scene_extra(&self, pack: &ScenePack, values: &Values) -> Vec<(String, Arc<RgbaImage>)> {
        let load = |file: String| match self.scene_file(&file) {
            Ok(decoded) => Some((file, decoded)),
            Err(e) => {
                tracing::warn!("cannot use the file chosen for a scene property: {}", e.root_cause());
                tracing::debug!("{e:#}");
                None
            }
        };
        pack.files(values).into_iter().filter_map(load).collect()
    }
}
