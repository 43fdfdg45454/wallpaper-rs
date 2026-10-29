use crate::compositor::Reel;
use crate::daemon::Daemon;
use crate::decode::decode;
use crate::sceneprops::Props;
use crate::store::private_dir;
use image::{ImageReader, RgbaImage};
use serde_json::Value as Json;
use std::collections::BTreeMap;
use std::io::{Cursor, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use wprs_render::SceneAssets;
use wprs_scene::{Control, Scene, Uses, Values};
use wprs_video::Bank;

const IMAGES: [&str; 6] = ["png", "jpg", "jpeg", "webp", "gif", "bmp"];
const FONTS: [&str; 5] = ["ttf", "otf", "ttc", "otc", "woff2"];
const VIDEOS: [&str; 3] = ["webm", "mp4", "mkv"];
const DECODED: u64 = 512 << 20;

pub struct ScenePack {
    pub path: PathBuf,
    pub props: Arc<Props>,
    pub locale: String,
    pub scene: Arc<Scene>,
    pub assets: SceneAssets,
    pub videos: Vec<(String, Reel)>,
    pub sounds: Vec<(String, PathBuf)>,
    pub clips: Bank,
    pub lengths: BTreeMap<String, f32>,
    pub scripts: Vec<(String, String)>,
    pub labels: Json,
    pub inputs: Option<Uses>,
    pub key: String,
}

pub type Extra = (Vec<(String, Arc<RgbaImage>)>, Vec<(String, Reel)>);

#[cfg(test)]
impl ScenePack {
    pub fn bare(
        scene: Scene,
        properties: BTreeMap<String, wprs_scene::Property>,
        sounds: Vec<(String, PathBuf)>,
    ) -> ScenePack {
        let (path, id, version, title, languages, presets) = Default::default();
        let props =
            Arc::new(Props { path, id, version, title, kind: "scene", languages, properties, presets });
        let (path, locale, assets, videos, clips, scripts, key) = Default::default();
        let labels = Json::Object(Default::default());
        ScenePack {
            path,
            props,
            locale,
            scene: Arc::new(scene),
            assets,
            videos,
            sounds,
            clips,
            lengths: BTreeMap::new(),
            scripts,
            labels,
            inputs: None,
            key,
        }
    }
}

pub(crate) fn streamed(scene: &Scene, id: &str) -> bool {
    scene.sounds().any(|s| s.stream && s.clips().any(|c| c == id))
}

pub(crate) fn bank(scene: &Scene, sounds: &[(String, PathBuf)]) -> (Bank, BTreeMap<String, f32>) {
    let files = sounds.iter().map(|(id, path)| (path.clone(), streamed(scene, id))).collect();
    let probed =
        sounds.iter().filter_map(|(id, path)| Some((id.clone(), wprs_video::probe_sound(path).ok()?)));
    (Bank::new(files), probed.collect())
}

pub(crate) fn listens(scene: &Scene, scripts: &[(String, String)]) -> bool {
    let uses = wprs_scene::needs(scene);
    let read = |word| scripts.iter().any(|(_, source)| source.contains(word));
    uses.audio || uses.cursor || read("audio") || read("cursor")
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
        let mut package = crate::decode::open(wpak)?;
        anyhow::ensure!(
            package.manifest.kind == wprs_format::Kind::Scene,
            "{} is not a scene package",
            wpak.display()
        );
        let scene = package.scene()?;
        let (mut assets, mut videos, mut sounds) = (SceneAssets::default(), Vec::new(), Vec::new());
        let mut decoded = wprs_render::Pictures::default();
        for (id, asset) in &scene.assets {
            let ext = kind(&asset.path);
            if IMAGES.contains(&ext.as_str()) {
                let read = || Ok(package.read_file(&asset.path)?);
                let total = decoded.add::<anyhow::Error>((&mut assets, id, asset), read, image)?;
                anyhow::ensure!(
                    total <= DECODED,
                    "the images of {} are too large once decoded",
                    wpak.display()
                );
            } else if FONTS.contains(&ext.as_str()) {
                assets.fonts.insert(id.clone(), Arc::new(package.read_file(&asset.path)?));
            } else if VIDEOS.contains(&ext.as_str()) || scene.sounds().any(|s| s.clips().any(|c| c == id)) {
                let blob = self.blob(wpak, stamp.0, stamp.1).with_extension(format!("{id}.{ext}"));
                if !blob.is_file() {
                    let dir = blob.parent().unwrap_or(Path::new("."));
                    private_dir(dir)?;
                    let mut file = tempfile::Builder::new().prefix(".tmp-").tempfile_in(dir)?;
                    file.write_all(&package.read_file(&asset.path)?)?;
                    file.persist(&blob)?;
                }
                match VIDEOS.contains(&ext.as_str()) {
                    true => videos.push((id.clone(), Reel::plain(blob))),
                    false => sounds.push((id.clone(), blob)),
                }
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
        let labels = wprs_scene::labels(&properties, &languages);
        let (clips, lengths) = bank(&scene, &sounds);
        let (id, version, title, presets) =
            (m.id.clone(), m.version.clone(), m.title.pick(&languages), m.presets);
        let (path, inputs) = (wpak.to_owned(), package.manifest.declared());
        let key = if listens(&scene, &scripts) { self.package_key(wpak, &meta)? } else { String::new() };
        let props =
            Props { path: path.clone(), id, version, title, kind: "scene", languages, properties, presets };
        let (scene, props) = (Arc::new(scene), Arc::new(props));
        let pack = Arc::new(ScenePack {
            path,
            props,
            locale,
            scene,
            assets,
            videos,
            sounds,
            clips,
            lengths,
            scripts,
            labels,
            inputs,
            key,
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

    pub fn scene_extra(&self, pack: &ScenePack, values: &Values) -> Extra {
        let (mut images, mut reels) = (Vec::new(), Vec::new());
        for (id, prop) in &pack.props.properties {
            let Some(chosen) = values.get(id).and_then(Json::as_str).filter(|v| v.starts_with('/')) else {
                continue;
            };
            let keys = match &prop.control {
                Control::File { default_asset: Some(asset), .. } => vec![asset.clone(), chosen.to_owned()],
                _ => vec![chosen.to_owned()],
            };
            let file = match prop.control {
                Control::Folder { .. } => match crate::picks::pick(Path::new(chosen), prop) {
                    Some(file) => file,
                    None => continue,
                },
                _ => PathBuf::from(chosen),
            };
            if wprs_video::is_video(&file) {
                reels.extend(keys.into_iter().map(|key| (key, Reel::plain(file.clone()))));
                continue;
            }
            match self.scene_file(&file.to_string_lossy()) {
                Ok(decoded) => images.extend(keys.into_iter().map(|key| (key, decoded.clone()))),
                Err(e) => {
                    tracing::warn!("cannot use the file chosen for a scene property: {}", e.root_cause());
                    tracing::debug!("{e:#}");
                }
            }
        }
        (images, reels)
    }
}
