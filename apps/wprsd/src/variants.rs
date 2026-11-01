use crate::daemon::Daemon;
use crate::error::{Result, failed, not_found};
use crate::sceneprops::stored;
use semver::{Version, VersionReq};
use std::io::Write;
use std::os::unix::fs::MetadataExt;
use wprs_library::Item;

fn version(item: &Item) -> Option<Version> {
    Version::parse(&item.package.as_ref()?.version).ok()
}

impl Daemon {
    pub(crate) fn apply_variant(&self, item: &Item, output: &str, fit: &str) -> Result<()> {
        let mut package = crate::decode::open(&item.path).map_err(|e| failed(format!("{e:#}")))?;
        let preset = package.preset()?;
        let base = self.base_for(&preset.base)?;
        let props = self.package_props(&base.path).map_err(|e| failed(format!("{e:#}")))?;
        let chosen = wprs_format::values::chosen(&preset, &props.properties);
        chosen.warnings.iter().for_each(|w| tracing::warn!(preset = package.manifest.id, "{w}"));
        let (meta, mut values) = (std::fs::metadata(&item.path)?, chosen.values);
        for (id, path) in chosen.files {
            let ext = path.rsplit_once('.').map_or("bin", |(_, e)| e);
            let blob = self.blob(&item.path, meta.mtime(), meta.len()).with_extension(format!("{id}.{ext}"));
            if !blob.is_file() {
                let dir = blob.parent().unwrap_or(std::path::Path::new("."));
                crate::store::private_dir(dir)?;
                let mut file = tempfile::Builder::new().prefix(".tmp-").tempfile_in(dir)?;
                file.write_all(&package.read_file(&path)?)?;
                file.persist(&blob).map_err(|e| e.error)?;
            }
            values.insert(id, serde_json::json!(blob));
        }
        let output = crate::apply::target_output(output)?;
        self.store_values(&output, &props, |kept| {
            kept.extend(values.iter().filter_map(|(k, v)| Some((k.clone(), stored(v)?))));
        })?;
        self.apply(&base.uid, &output, fit, "")
    }

    pub(crate) fn base_for(&self, base: &wprs_format::Dependency) -> Result<Item> {
        let wanted = VersionReq::parse(&base.version).map_err(|e| failed(e.to_string()))?;
        let usable = |i: &Item| {
            let kind = i.package.as_ref().map(|p| p.kind.as_str());
            kind.is_some_and(|k| k != "preset" && k != "assets")
                && version(i).is_some_and(|v| wanted.matches(&v))
        };
        let found = self.store.read(|lib| lib.packages_of(&base.id))?;
        let best = found.into_iter().filter(usable).max_by_key(version);
        let missing =
            || not_found(format!("this preset needs {} {}; install it first", base.id, base.version));
        best.ok_or_else(missing)
    }
}

impl Daemon {
    pub(crate) fn install_needed(
        self: &std::sync::Arc<Self>,
        record: &wprs_format::Installed,
        (stale, downgrade): (bool, bool),
    ) {
        let needed = wprs_format::peek(&record.path).map(|m| m.dependencies().to_vec()).unwrap_or_default();
        for dependency in needed.iter().filter(|d| self.base_for(d).is_err()) {
            let spec = format!("{}/{}", record.repo_id, dependency.id);
            if let Err(e) = self.install(&spec, stale, downgrade) {
                tracing::warn!(
                    package = dependency.id,
                    "cannot install a package that {} needs: {e}",
                    record.id
                );
            }
        }
    }
}

impl Daemon {
    pub(crate) fn kin(
        &self,
        item: &Item,
    ) -> (Vec<wprs_core::model::PlaylistRef>, Option<wprs_core::model::PlaylistRef>) {
        let named = |i: Item| wprs_core::model::PlaylistRef { uid: i.uid, name: i.title };
        let Some(package) = item.package.as_ref() else { return (Vec::new(), None) };
        let variants = self.store.read(|lib| lib.variants(&package.id)).unwrap_or_default();
        let base = package.base.as_ref().and_then(|id| self.store.read(|lib| lib.packages_of(id)).ok());
        let base = base.and_then(|found| {
            found.into_iter().find(|i| i.package.as_ref().is_some_and(|p| p.kind != "preset"))
        });
        (variants.into_iter().map(named).collect(), base.map(named))
    }
}
