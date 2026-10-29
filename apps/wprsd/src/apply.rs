use crate::daemon::Daemon;
use crate::error::{Error, Result, failed, invalid, not_found};
use crate::jobs::now;
use crate::store::private_dir;
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use wprs_core::Fit;
use wprs_core::config::parse_hex_color;
use wprs_core::dbus::error::UNCONFIRMED;
use wprs_core::model::PresetValue;
use wprs_library::{Kind, Library};

pub fn target_output(output: &str) -> Result<String> {
    let output = output.trim();
    if output.len() > 128 || output.chars().any(char::is_control) || output.starts_with('@') {
        return Err(invalid("invalid output name"));
    }
    Ok(output.to_owned())
}

pub fn parse_fit(fit: &str) -> Result<Option<Fit>> {
    let fit = Some(fit.trim()).filter(|f| !f.is_empty()).map(str::parse::<Fit>).transpose();
    fit.map_err(|e| invalid(e.to_string()))
}

fn find_preset(lib: &Library, item: &str, key: &str) -> Result<Option<(String, PresetValue)>> {
    let key = key.trim();
    if key.is_empty() {
        return Ok(None);
    }
    let named = |list: Vec<wprs_library::Preset>| {
        list.into_iter().find(|p| p.name.eq_ignore_ascii_case(key) && p.package().is_none())
    };
    let found = match lib.preset(key) {
        Ok(preset) => Some(preset),
        Err(_) => named(lib.presets(Some(item))?).or(named(lib.presets(None)?)),
    };
    let found = crate::items::preset(found.ok_or_else(|| not_found(format!("preset {key:?} not found")))?);
    Ok(Some((found.uid, found.value)))
}

pub enum Media {
    Picture(PathBuf),
    Video(PathBuf, Option<Arc<crate::sceneprops::Props>>),
    Scene(Arc<crate::scenes::ScenePack>),
}

pub fn materialize(wpak: &Path, blob: &Path) -> anyhow::Result<bool> {
    let mut package = crate::decode::open(wpak)?;
    let video = match package.manifest.kind {
        wprs_format::Kind::Image => false,
        wprs_format::Kind::Video => true,
        kind => anyhow::bail!("{kind} packages are not files"),
    };
    if !blob.is_file() {
        let dir = blob.parent().unwrap_or(Path::new("."));
        private_dir(dir)?;
        let mut file = tempfile::Builder::new().prefix(".tmp-").tempfile_in(dir)?;
        file.write_all(&package.read_file(&package.manifest.entry.clone())?)?;
        file.persist(blob)?;
    }
    Ok(video)
}

pub type Look = (Option<Fit>, Option<[u8; 3]>);

fn display(path: &Path) -> wprs_format::Display {
    let package = Kind::detect(path) == Some(Kind::Package);
    let manifest = package.then(|| wprs_format::peek(path).ok()).flatten();
    manifest.and_then(|m| m.display).unwrap_or_default()
}

pub fn author(path: &Path) -> Look {
    let display = display(path);
    let span = display.span == Some(wprs_format::Span::AllOutputs);
    (display.fit.or(span.then_some(Fit::Span)), display.background.as_deref().and_then(parse_hex_color))
}

pub fn look((fit, background): Look, author: impl FnOnce() -> Look) -> (Fit, [u8; 3]) {
    let (by_fit, by_background) = if fit.is_some() && background.is_some() { (None, None) } else { author() };
    (fit.or(by_fit).unwrap_or_default(), background.or(by_background).unwrap_or_default())
}

impl Daemon {
    pub fn blob(&self, path: &Path, mtime: i64, size: u64) -> PathBuf {
        let identity = format!("{}\0{mtime}\0{size}", path.to_string_lossy());
        let name = wprs_format::sha256(identity.as_bytes()).unwrap_or_default();
        self.store.cache.join("blobs").join(name)
    }

    pub fn media(&self, path: PathBuf) -> anyhow::Result<Media> {
        if Kind::detect(&path) != Some(Kind::Package) {
            return Ok(if wprs_video::is_video(&path) {
                Media::Video(path, None)
            } else {
                Media::Picture(path)
            });
        }
        let revoked = self.state().installed.iter().any(|i| i.path == path && i.revoked.is_some());
        anyhow::ensure!(!revoked, "the package is revoked");
        if wprs_format::peek(&path)?.kind == wprs_format::Kind::Scene {
            return self.scene_pack(&path).map(Media::Scene);
        }
        let meta = std::fs::metadata(&path)?;
        let blob = self.blob(&path, meta.mtime(), meta.len());
        Ok(match materialize(&path, &blob)? {
            true => Media::Video(blob, self.video_props(&path).ok()),
            false => Media::Picture(blob),
        })
    }

    pub fn drop_blobs(&self, path: &Path, mtime: i64, size: u64) {
        let blob = self.blob(path, mtime, size);
        let name = blob.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let entries = std::fs::read_dir(self.store.cache.join("blobs")).into_iter().flatten().flatten();
        let ours =
            |e: &std::fs::DirEntry| e.file_name().to_string_lossy().split('.').next() == Some(name.as_str());
        entries.filter(ours).for_each(|e| drop(std::fs::remove_file(e.path())));
    }

    pub fn apply(&self, uid: &str, output: &str, fit: &str, preset: &str) -> Result<()> {
        let (output, fit) = (target_output(output)?, parse_fit(fit)?);
        let (item, preset) =
            self.store.read(|lib| Ok::<_, Error>((lib.item(uid)?, find_preset(lib, uid, preset)?)))?;
        if item.missing {
            return Err(failed(format!("the file of {uid} is missing")));
        }
        if let Some(error) = &item.meta.error {
            return Err(failed(error.clone()));
        }
        if item.package.as_ref().is_some_and(|p| p.asks("photosensitive")) {
            let message = "this wallpaper flashes or shows fast patterns that can trigger seizures in people \
                with photosensitive epilepsy; the user has not allowed it yet";
            return Err(Error::Named(UNCONFIRMED, message.into()));
        }
        if let Some(package) = self.installed(&item).filter(|i| i.revoked.is_some()) {
            let reason =
                serde_json::to_value(package.revoked).ok().and_then(|v| v.as_str().map(String::from));
            let reason = reason.unwrap_or_default();
            return Err(failed(format!("{} {} is revoked ({reason})", package.id, package.version)));
        }
        let (fit, background, preset) = match preset {
            Some((uid, PresetValue { fit: preset_fit, background, .. })) => {
                (fit.or(preset_fit), background, Some(uid))
            }
            None => (fit, None, None),
        };
        let fit = fit.or_else(|| display(&item.path).fit);
        let _decoded = self.check_media(&item.path)?;
        self.change_output(&output, |c| {
            c.set_image(&output, item.path.clone(), fit);
            background.as_deref().map_or(Ok(()), |color| c.set_background(&output, color))?;
            match output.as_str() {
                "" => c.default.preset = preset,
                name => c.outputs.entry(name.into()).or_default().preset = preset,
            }
            Ok(())
        })?;
        let shown: Vec<String> = self
            .state()
            .outputs
            .keys()
            .filter(|o| !crate::drafts::virtual_output(o))
            .filter(|o| output.is_empty() || **o == output)
            .cloned()
            .collect();
        self.store.write(|lib| shown.iter().try_for_each(|o| lib.mark_used(uid, o, now())))?;
        self.changes.changed("library", vec![uid.to_owned()]);
        Ok(())
    }

    pub fn tidy(self: &Arc<Self>) {
        self.offer_examples();
        if self.store.write(|lib| lib.reread()).is_ok_and(|n| n > 0) {
            self.library_changed(Vec::new());
        }
        self.thumbs.prune(u64::from(self.state().config.library.thumb_cache_mb) << 20);
        let packages = self.store.read(|lib| lib.search("kind:package", now(), 0, u32::MAX));
        let Ok(packages) = packages else { return };
        let kept: HashSet<PathBuf> = packages.iter().map(|i| self.blob(&i.path, i.mtime, i.size)).collect();
        let blobs = std::fs::read_dir(self.store.cache.join("blobs")).into_iter().flatten().flatten();
        let base = |e: &std::fs::DirEntry| {
            e.path().with_file_name(e.file_name().to_string_lossy().split('.').next().unwrap_or_default())
        };
        blobs.filter(|e| !kept.contains(&base(e))).for_each(|e| drop(std::fs::remove_file(e.path())));
        if self.store.read(|lib| lib.unanalyzed(1)).is_ok_and(|found| !found.is_empty()) {
            self.analyze_job();
        }
    }

    pub fn refresh_items(&self) {
        let paths: Vec<PathBuf> = {
            let st = self.state();
            let sections = std::iter::once(&st.config.default).chain(st.config.outputs.values());
            sections.filter_map(|s| s.image.clone()).collect()
        };
        let find = |lib: &Library| -> Result<HashMap<PathBuf, String>> {
            let uid = |p: &PathBuf| Some((p.clone(), lib.resolve_item(p.to_str()?).ok()?));
            Ok(paths.iter().filter_map(uid).collect())
        };
        let found = self.store.read(find).unwrap_or_default();
        let changed = {
            let mut st = self.state();
            std::mem::replace(&mut st.items, found) != st.items
        };
        if changed {
            self.changes.changed("state", Vec::new());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_author_look_fills_only_what_the_user_left_open() {
        let author = || (Some(Fit::Span), Some([255, 0, 0]));
        assert_eq!(look((None, None), author), (Fit::Span, [255, 0, 0]));
        assert_eq!(look((None, Some([1, 2, 3])), author), (Fit::Span, [1, 2, 3]));
        assert_eq!(look((Some(Fit::Center), None), author), (Fit::Center, [255, 0, 0]));
        assert_eq!(look((Some(Fit::Center), Some([1, 2, 3])), || unreachable!()), (Fit::Center, [1, 2, 3]));
        assert_eq!(look((None, None), || (None, None)), (Fit::default(), [0, 0, 0]));
    }

    #[test]
    fn the_author_look_reads_only_the_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        std::fs::create_dir(&source).unwrap();
        let png = image::RgbaImage::from_pixel(8, 8, image::Rgba([0, 0, 255, 255]));
        ["image.png", "preview.png"].iter().for_each(|name| png.save(source.join(name)).unwrap());
        let manifest = serde_json::json!({"schema": wprs_format::SCHEMA, "id": "org.example.look", "version": "1.0.0",
            "title": "Look", "license": "CC0-1.0", "kind": "image", "entry": "image.png", "content": {"rating": "general"},
            "photosensitive": false, "origin": {"kind": "original"}, "previews": {"thumbnail": "preview.png"},
            "requires": {"features": ["flip"]}, "display": {"flip": "horizontal", "background": "#ff0000"}});
        std::fs::write(source.join("manifest.json"), manifest.to_string()).unwrap();
        let wpak = dir.path().join("look.wpak");
        wprs_format::build(&source, &wpak, &wprs_format::Limits::default()).unwrap();
        assert!(crate::decode::open(&wpak).is_err());
        assert_eq!(author(&wpak), (None, Some([255, 0, 0])));
    }
}
