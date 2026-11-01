use crate::compositor::Reel;
use crate::daemon::Daemon;
use crate::decode::decode;
use crate::sceneprops::Props;
use image::{ImageReader, RgbaImage};
use serde_json::Value as Json;
use std::collections::BTreeMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use wprs_render::{Pixels, SceneAssets};
use wprs_scene::{Control, Scene, Uses, Values};
use wprs_video::Bank;

mod load;

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

pub type Extra = (Vec<(String, Arc<Pixels>)>, Vec<(String, Reel)>);

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

pub(crate) fn filmed(lengths: &mut BTreeMap<String, f32>, videos: &[(String, Reel)]) {
    let found = videos.iter().filter_map(|(id, reel)| Some((id.clone(), wprs_scene::film_file(&reel.path)?)));
    lengths.extend(found);
}

pub(crate) fn listens(scene: &Scene, scripts: &[(String, String)]) -> bool {
    let uses = wprs_scene::needs(scene);
    uses.audio || uses.cursor || wprs_scene::script_inputs(scene, scripts).contains(&true)
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
    pub fn scene_file(&self, file: &str) -> anyhow::Result<Arc<RgbaImage>> {
        let path = Path::new(file);
        anyhow::ensure!(path.is_file(), "it is not a file");
        self.load_image(path, std::fs::metadata(path)?.modified().ok(), None)
    }

    pub(crate) fn scene_pixels(&self, file: &Path) -> anyhow::Result<Arc<Pixels>> {
        let modified = std::fs::metadata(file)?.modified().ok();
        let cached =
            self.state().pixels.get(file).filter(|(m, _)| *m == modified).and_then(|(_, w)| w.upgrade());
        if let Some(pixels) = cached {
            return Ok(pixels);
        }
        let (path, edge) = (file.to_owned(), self.compositor.max_edge());
        let reload = move || wprs_render::decode_image(&path, edge).ok();
        let pixels = Pixels::reloadable(self.scene_file(&file.to_string_lossy())?, reload);
        let mut st = self.state();
        st.pixels.retain(|_, (_, weak)| weak.strong_count() > 0);
        st.pixels.insert(file.to_owned(), (modified, Arc::downgrade(&pixels)));
        Ok(pixels)
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
            match self.scene_pixels(&file) {
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
