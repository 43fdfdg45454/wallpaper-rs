use crate::daemon::Daemon;
use crate::decode::SCENE;
use crate::error::{Error, Result, failed, invalid, not_found};
use crate::jobs::now;
use crate::store::private_dir;
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use wprs_core::Fit;
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
    let named = |list: Vec<wprs_library::Preset>| list.into_iter().find(|p| p.name.eq_ignore_ascii_case(key));
    let found = match lib.preset(key) {
        Ok(preset) => Some(preset),
        Err(_) => named(lib.presets(Some(item))?).or(named(lib.presets(None)?)),
    };
    let found = crate::items::preset(found.ok_or_else(|| not_found(format!("preset {key:?} not found")))?);
    Ok(Some((found.uid, found.value)))
}

pub fn materialize(wpak: &Path, blob: &Path) -> anyhow::Result<bool> {
    let mut package = wprs_format::open(wpak, &wprs_format::Limits::default())?;
    let video = match package.manifest.kind {
        wprs_format::Kind::Image => false,
        wprs_format::Kind::Video => true,
        wprs_format::Kind::Scene => anyhow::bail!(SCENE),
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

impl Daemon {
    pub fn blob(&self, path: &Path, mtime: i64, size: u64) -> PathBuf {
        let identity = format!("{}\0{mtime}\0{size}", path.to_string_lossy());
        let name = wprs_format::sha256(identity.as_bytes()).unwrap_or_default();
        self.store.cache.join("blobs").join(name)
    }

    pub fn media(&self, path: PathBuf) -> anyhow::Result<(PathBuf, bool)> {
        if Kind::detect(&path) != Some(Kind::Package) {
            let video = wprs_video::is_video(&path);
            return Ok((path, video));
        }
        let revoked = self.state().installed.iter().any(|i| i.path == path && i.revoked.is_some());
        anyhow::ensure!(!revoked, "the package is revoked");
        let meta = std::fs::metadata(&path)?;
        let blob = self.blob(&path, meta.mtime(), meta.len());
        let video = materialize(&path, &blob)?;
        Ok((blob, video))
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
        if let Some(package) = self.installed(&item).filter(|i| i.revoked.is_some()) {
            let reason =
                serde_json::to_value(package.revoked).ok().and_then(|v| v.as_str().map(String::from));
            let reason = reason.unwrap_or_default();
            return Err(failed(format!("{} {} is revoked ({reason})", package.id, package.version)));
        }
        let (fit, background, preset) = match preset {
            Some((uid, PresetValue { fit: preset_fit, background })) => {
                (fit.or(preset_fit), background, Some(uid))
            }
            None => (fit, None, None),
        };
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
        let shown: Vec<String> =
            self.state().outputs.keys().filter(|o| output.is_empty() || **o == output).cloned().collect();
        self.store.write(|lib| shown.iter().try_for_each(|o| lib.mark_used(uid, o, now())))?;
        self.changes.changed("library", vec![uid.to_owned()]);
        Ok(())
    }

    pub fn tidy(self: &Arc<Self>) {
        self.thumbs.prune(u64::from(self.state().config.library.thumb_cache_mb) << 20);
        let packages = self.store.read(|lib| lib.search("kind:package", now(), 0, u32::MAX));
        let Ok(packages) = packages else { return };
        let kept: HashSet<PathBuf> = packages.iter().map(|i| self.blob(&i.path, i.mtime, i.size)).collect();
        let blobs = std::fs::read_dir(self.store.cache.join("blobs")).into_iter().flatten().flatten();
        blobs.filter(|e| !kept.contains(&e.path())).for_each(|e| drop(std::fs::remove_file(e.path())));
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
