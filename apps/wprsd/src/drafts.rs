use crate::compositor::Reel;
use crate::daemon::{Daemon, output_name};
use crate::error::invalid;
use crate::sceneprops::Props;
use crate::scenes::ScenePack;
use anyhow::{Context, Result, ensure};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};
use wprs_format::{Kind, Manifest, Platform, check_path, features, valid_id};
use wprs_render::SceneAssets;
use wprs_scene::{Clip, Lut, MAX_LUT_BYTES, MAX_LUT_TOTAL, MAX_SOUND_BYTES, MAX_SOUND_TOTAL, Scene};
use wprs_video::Bank;

const LIFE: Duration = Duration::from_secs(60);
const MAX_DOCUMENT: usize = 9 << 20;
const DECODED: u64 = 512 << 20;
const PREVIEW: &str = "preview:";

pub fn virtual_output(name: &str) -> bool {
    name.starts_with(PREVIEW)
}

pub(crate) struct Draft {
    pub(crate) pack: Arc<ScenePack>,
    pub(crate) hold: Option<f64>,
    document: String,
    at: Instant,
}

fn kind(path: &str) -> String {
    Path::new(path).extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase()
}

impl Daemon {
    fn draft_pack(&self, dir: &Path, document: &str) -> Result<ScenePack> {
        let doc: Value = serde_json::from_str(document).map_err(|e| invalid(e.to_string()))?;
        let manifest: Manifest =
            serde_json::from_value(doc["manifest"].clone()).map_err(|e| invalid(e.to_string()))?;
        ensure!(valid_id(&manifest.id), invalid("the manifest id is not a name like org.example.name"));
        manifest.check_properties().map_err(|e| invalid(e.to_string()))?;
        ensure!(manifest.kind == Kind::Scene, invalid("the manifest is not a scene manifest"));
        let scene = Scene::load(&doc["scene"].to_string(), &manifest.properties)
            .map_err(|e| invalid(e.to_string()))?;
        let facts = features::facts(&manifest, Some(&scene), None);
        features::playable(&manifest, &facts, Platform::Desktop).map_err(|e| invalid(e.to_string()))?;
        let dir = std::fs::canonicalize(dir).with_context(|| format!("cannot find {}", dir.display()))?;
        let file = |path: &str| -> Result<PathBuf> {
            check_path(path).map_err(|e| invalid(e.to_string()))?;
            let found =
                std::fs::canonicalize(dir.join(path)).map_err(|_| invalid(format!("{path} is missing")))?;
            ensure!(
                found.starts_with(&dir) && found.is_file(),
                invalid(format!("{path} is not a file of the folder"))
            );
            Ok(found)
        };
        let (mut assets, mut videos, mut sounds) = (SceneAssets::default(), Vec::new(), Vec::new());
        let (mut decoded, mut seconds, mut tables) = (0u64, 0f32, 0u64);
        for (id, asset) in &scene.assets {
            let (path, ext) = (file(&asset.path)?, kind(&asset.path));
            let size = std::fs::metadata(&path)?.len();
            match ext.as_str() {
                "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" => {
                    let image = self.load_image(&path, std::fs::metadata(&path)?.modified().ok(), None)?;
                    decoded += u64::from(image.width()) * u64::from(image.height()) * 4;
                    ensure!(
                        decoded <= DECODED,
                        invalid("the images of the scene are too large once decoded")
                    );
                    assets.images.insert(id.clone(), image);
                }
                "ttf" | "otf" => drop(assets.fonts.insert(id.clone(), Arc::new(std::fs::read(&path)?))),
                "webm" | "mp4" | "mkv" => videos.push((id.clone(), Reel::plain(path))),
                "cube" => {
                    ensure!(size <= MAX_LUT_BYTES as u64, invalid(format!("{} is too large", asset.path)));
                    let lut = Lut::parse(&std::fs::read(&path)?).map_err(|e| invalid(e.to_string()))?;
                    tables += lut.data.len() as u64 * 12;
                    ensure!(tables <= MAX_LUT_TOTAL as u64, invalid("the color tables are too large"));
                    assets.luts.insert(id.clone(), Arc::new(lut));
                }
                _ if scene.sounds().any(|s| s.sound == *id) => {
                    ensure!(size <= MAX_SOUND_BYTES, invalid(format!("{} is too large", asset.path)));
                    let clip = Clip::probe(&asset.path, &std::fs::read(&path)?)
                        .map_err(|e| invalid(format!("{} {e}", asset.path)))?;
                    seconds += clip.seconds;
                    ensure!(seconds <= MAX_SOUND_TOTAL, invalid("the sounds of the scene last too long"));
                    sounds.push((id.clone(), path));
                }
                _ => {}
            }
        }
        let scripts = scene.scripts.iter().map(|s| {
            let text = std::fs::read_to_string(file(&s.path)?)
                .map_err(|_| invalid(format!("{} is not text", s.path)))?;
            Ok((s.path.clone(), text))
        });
        let scripts = scripts.collect::<Result<Vec<_>>>()?;
        let (m, inputs) = (manifest.localized(), manifest.declared());
        let user = wprs_core::locale::languages();
        let (locale, languages) = (m.locale_in(&user), m.languages(&user));
        let properties = scene.properties(&m.properties, &crate::words::volume());
        let labels = wprs_scene::labels(&properties, &languages);
        let clips = Bank::new(sounds.iter().map(|(_, path)| path.clone()).collect());
        let (id, version, title) = (m.id.clone(), m.version.clone(), m.title.pick(&languages));
        let (path, presets) = (dir.clone(), m.presets);
        let props = Props { path, id, version, title, kind: "scene", languages, properties, presets };
        let (props, scene) = (Arc::new(props), Arc::new(scene));
        Ok(ScenePack {
            path: dir,
            props,
            locale,
            scene,
            assets,
            videos,
            sounds,
            clips,
            scripts,
            labels,
            inputs,
        })
    }

    pub fn preview_scene(
        self: &Arc<Self>,
        output: &str,
        dir: &Path,
        document: &str,
        time: f64,
    ) -> Result<()> {
        let outputs: Vec<String> = match output.trim() {
            "" => self.state().outputs.keys().filter(|o| !virtual_output(o)).cloned().collect(),
            name => vec![output_name(name)?],
        };
        ensure!(document.len() <= MAX_DOCUMENT, invalid("the scene is too large"));
        let hold = (time >= 0.0 && time.is_finite()).then_some(time.min(600.0));
        let pack = match document.trim() {
            "" => None,
            _ => {
                let dir =
                    std::fs::canonicalize(dir).with_context(|| format!("cannot find {}", dir.display()))?;
                let found = self
                    .state()
                    .drafts
                    .values()
                    .find(|d| d.document == document && d.pack.path == dir)
                    .map(|d| d.pack.clone());
                Some(found.map_or_else(|| self.draft_pack(&dir, document).map(Arc::new), Ok)?)
            }
        };
        let started = {
            let mut st = self.state();
            let mut started = false;
            for output in &outputs {
                match &pack {
                    Some(pack) => {
                        let draft =
                            Draft { pack: pack.clone(), hold, document: document.into(), at: Instant::now() };
                        started |= st.drafts.insert(output.clone(), draft).is_none();
                    }
                    None => drop(st.drafts.remove(output)),
                }
            }
            started
        };
        if started {
            tokio::spawn(expire(Arc::downgrade(self)));
        }
        outputs.iter().for_each(|output| drop(self.configure_output(output)));
        Ok(())
    }
}

async fn expire(daemon: Weak<Daemon>) {
    while let Some(d) = daemon.upgrade() {
        let now = Instant::now();
        let (gone, next) = {
            let mut st = d.state();
            let gone: Vec<String> =
                st.drafts.iter().filter(|(_, x)| x.at + LIFE <= now).map(|(o, _)| o.clone()).collect();
            gone.iter().for_each(|o| drop(st.drafts.remove(o)));
            (gone, st.drafts.values().map(|x| x.at + LIFE).min())
        };
        if !gone.is_empty() {
            let daemon = d.clone();
            tokio::task::spawn_blocking(move || gone.iter().for_each(|o| drop(daemon.configure_output(o))));
        }
        let Some(next) = next else { return };
        drop(d);
        tokio::time::sleep_until(next.into()).await;
    }
}
