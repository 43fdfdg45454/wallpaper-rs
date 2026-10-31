use super::{DECODED, kind};
use crate::compositor::Reel;
use crate::daemon::Daemon;
use crate::error::invalid;
use crate::sceneprops::Props;
use crate::scenes::ScenePack;
use anyhow::{Context, Result, ensure};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use wprs_format::{Kind, Manifest, Platform, check_path, features, valid_id};
use wprs_render::SceneAssets;
use wprs_scene::{
    Clip, Lut, MAX_LUT_BYTES, MAX_LUT_TOTAL, MAX_SOUND_BYTES, MAX_SOUND_TOTAL, MAX_STREAM_BYTES,
    MAX_STREAM_SECONDS, MAX_STREAM_TOTAL, Scene,
};

impl Daemon {
    pub(super) fn draft_pack(&self, dir: &Path, document: &str, trusted: bool) -> Result<ScenePack> {
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
        let (mut decoded, mut seconds, mut streamed, mut tables) = (0u64, 0f32, 0f32, 0u64);
        for (id, asset) in &scene.assets {
            let (path, ext) = (file(&asset.path)?, kind(&asset.path));
            let size = std::fs::metadata(&path)?.len();
            match ext.as_str() {
                "gif" | "webp" if asset.animated => {
                    let (atlas, durations) = wprs_render::animation(&std::fs::read(&path)?)
                        .map_err(|e| invalid(format!("{} {e}", asset.path)))?;
                    decoded += u64::from(atlas.width()) * u64::from(atlas.height()) * 4;
                    ensure!(
                        decoded <= DECODED,
                        invalid("the images of the scene are too large once decoded")
                    );
                    assets.sheets.insert(id.clone(), durations);
                    assets.images.insert(id.clone(), wprs_render::Pixels::new(atlas));
                }
                "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" => {
                    let image = self.scene_pixels(&path)?;
                    decoded += u64::from(image.size().0) * u64::from(image.size().1) * 4;
                    ensure!(
                        decoded <= DECODED,
                        invalid("the images of the scene are too large once decoded")
                    );
                    assets.images.insert(id.clone(), image);
                }
                "ttf" | "otf" | "ttc" | "otc" | "woff2" => {
                    drop(assets.fonts.insert(id.clone(), Arc::new(std::fs::read(&path)?)))
                }
                "webm" | "mp4" | "mkv" => videos.push((id.clone(), Reel::plain(path))),
                "cube" => {
                    ensure!(size <= MAX_LUT_BYTES as u64, invalid(format!("{} is too large", asset.path)));
                    let lut = Lut::parse(&std::fs::read(&path)?).map_err(|e| invalid(e.to_string()))?;
                    tables += lut.data.len() as u64 * 12;
                    ensure!(tables <= MAX_LUT_TOTAL as u64, invalid("the color tables are too large"));
                    assets.luts.insert(id.clone(), Arc::new(lut));
                }
                _ if scene.sounds().any(|s| s.clips().any(|c| c == id)) => {
                    let stream = crate::scenes::streamed(&scene, id);
                    let most = if stream { MAX_STREAM_BYTES } else { MAX_SOUND_BYTES };
                    ensure!(size <= most, invalid(format!("{} is too large", asset.path)));
                    let probed = match stream {
                        true => wprs_video::probe_sound(&path).map_err(|e| e.to_string()),
                        false => Clip::probe(&asset.path, &std::fs::read(&path)?).map(|c| c.seconds),
                    };
                    let length = probed.map_err(|e| invalid(format!("{} {e}", asset.path)))?;
                    let (total, limit) = if stream {
                        (&mut streamed, MAX_STREAM_TOTAL as f32)
                    } else {
                        (&mut seconds, MAX_SOUND_TOTAL)
                    };
                    *total += if stream { size as f32 } else { length };
                    ensure!(*total <= limit, invalid("the sounds of the scene are too long or too large"));
                    ensure!(length <= MAX_STREAM_SECONDS, invalid(format!("{} lasts too long", asset.path)));
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
        let (clips, lengths) = crate::scenes::bank(&scene, &sounds);
        let (id, version, title) = (m.id.clone(), m.version.clone(), m.title.pick(&languages));
        let (path, presets) = (dir.clone(), m.presets);
        let props = Props { path, id, version, title, kind: "scene", languages, properties, presets };
        let key = match crate::scenes::listens(&scene, &scripts) {
            true => crate::inputs::draft_key(&dir, document, &scripts, trusted),
            false => String::new(),
        };
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
            lengths,
            scripts,
            labels,
            inputs,
            key,
        })
    }
}
