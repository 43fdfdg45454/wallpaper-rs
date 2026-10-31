use super::{DECODED, FONTS, IMAGES, ScenePack, VIDEOS, bank, image, kind, listens};
use crate::compositor::Reel;
use crate::daemon::Daemon;
use crate::sceneprops::Props;
use crate::store::private_dir;
use std::io::Write;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::sync::Arc;
use wprs_render::SceneAssets;

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
        let file = wpak.to_owned();
        assets.reloadable(&scene, move || {
            std::fs::metadata(&file).ok().filter(|m| (m.mtime(), m.len()) == stamp)?;
            let mut package = crate::decode::open(&file).ok()?;
            Some(move |path: &str| package.read_file(path).map_err(|e| e.to_string()))
        });
        let mut scripts = Vec::new();
        for script in &scene.scripts {
            let source = String::from_utf8(package.read_file(&script.path)?)
                .map_err(|_| anyhow::anyhow!("{} is not UTF-8 text", script.path))?;
            scripts.push((script.path.clone(), source));
        }
        let total: usize = scripts.iter().map(|(_, s)| s.len()).sum();
        anyhow::ensure!(
            total <= wprs_scene::MAX_SCRIPT_BYTES,
            "the scripts of {} take more than 1 MiB",
            wpak.display()
        );
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
}
