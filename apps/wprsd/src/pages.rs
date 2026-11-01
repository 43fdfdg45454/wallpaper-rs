use crate::daemon::Daemon;
use crate::sceneprops::Props;
use serde_json::Value as Json;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use wprs_core::model::Input;
use wprs_scene::{Control, Values};

mod helper;
mod kept;
mod shape;

pub use helper::{available, helper, unavailable};
pub use kept::within;
use shape::color;
pub use shape::shape;

pub const HIDES: [&str; 4] = ["hide_zone", "hide_language", "hide_gpu", "hide_storage"];
pub const PLAIN: &str = "en-US";

static CLEARED: std::sync::Mutex<std::collections::BTreeMap<String, u64>> =
    std::sync::Mutex::new(std::collections::BTreeMap::new());

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Privacy {
    pub zone: bool,
    pub language: bool,
    pub gpu: bool,
    pub storage: bool,
    pub network: bool,
}

pub struct WebPack {
    pub path: PathBuf,
    pub props: Arc<Props>,
    pub leveling: wprs_scene::Leveling,
    pub entry: String,
    pub preload: Option<String>,
    pub background: [u8; 3],
    pub network: Vec<String>,
    pub uses: [bool; 3],
    pub storage_kib: u32,
    pub key: String,
    pub shape: Json,
    pub plain: Json,
    pub own_volume: bool,
    pub max_fps: u32,
}

#[derive(Clone)]
pub struct PageSpec {
    pub pack: Arc<WebPack>,
    pub values: Values,
    pub privacy: Privacy,
    pub files: Vec<(String, PathBuf)>,
    pub folders: Vec<(String, PathBuf, bool)>,
    pub storage: Option<PathBuf>,
    pub cleared: u64,
}

impl PartialEq for PageSpec {
    fn eq(&self, other: &PageSpec) -> bool {
        let same = (&self.values, self.privacy, &self.files, &self.folders, &self.storage, self.cleared);
        let theirs =
            (&other.values, other.privacy, &other.files, &other.folders, &other.storage, other.cleared);
        self.pack.path == other.pack.path && same == theirs
    }
}

impl Daemon {
    pub fn web_pack(&self, wpak: &Path) -> anyhow::Result<Arc<WebPack>> {
        let package = crate::decode::open(wpak)?;
        let m = package.manifest.localized();
        anyhow::ensure!(m.kind == wprs_format::Kind::Web, "{} is not a web package", wpak.display());
        let languages = m.languages(&wprs_core::locale::languages());
        let (id, version, title) = (m.id.clone(), m.version.clone(), m.title.pick(&languages));
        let (mut properties, presets) = (m.properties.clone(), m.presets.clone());
        let authored = crate::sceneprops::Authored::of(&m);
        let own_volume = properties.contains_key(wprs_scene::VOLUME);
        let added = wprs_scene::volume_property(&crate::words::volume(), authored.playback.loudness());
        properties.entry(wprs_scene::VOLUME.into()).or_insert(added);
        let max_fps = authored.playback.frame_cap();
        let kind = "web";
        let props =
            Props { path: wpak.into(), id, version, title, kind, languages, properties, presets, authored };
        let web = m.web.clone().unwrap_or_default();
        let network = m.permissions.iter().flat_map(|p| &p.network).flat_map(|n| n.allow.clone()).collect();
        let has = |i: wprs_format::Input| m.inputs.contains(&i);
        let uses = [
            has(wprs_format::Input::Audio),
            has(wprs_format::Input::Pointer),
            has(wprs_format::Input::Clicks),
        ];
        let key = self.package_key(wpak, &std::fs::metadata(wpak)?)?;
        let (shape, plain) = (shape(&props, &props.languages), shape(&props, &m.languages(&[PLAIN.into()])));
        Ok(Arc::new(WebPack {
            path: wpak.into(),
            leveling: wprs_scene::Leveling::of(&props.properties),
            props: Arc::new(props),
            entry: m.entry.clone(),
            preload: web.preload,
            background: color(web.background.as_deref()),
            network,
            uses,
            storage_kib: web.storage_kib,
            key,
            shape,
            plain,
            own_volume,
            max_fps,
        }))
    }

    pub fn privacy(&self, pack: &WebPack) -> Privacy {
        let id = &pack.props.id;
        let hidden = |what: &str| self.store.read(|lib| lib.decided(id, what)).unwrap_or(false);
        let network = crate::inputs::hub()
            .is_some_and(|hub| crate::compositor::lock(&hub.control).has(id, &pack.key, Input::Network));
        let [zone, language, gpu, storage] = HIDES.map(|what| !hidden(what));
        Privacy { zone, language, gpu, storage, network }
    }

    pub fn page_spec(&self, pack: Arc<WebPack>, values: Values) -> PageSpec {
        let chosen = |id: &String| values.get(id).and_then(Json::as_str).filter(|v| v.starts_with('/'));
        let (mut files, mut folders) = (Vec::new(), Vec::new());
        for (id, prop) in &pack.props.properties {
            let Some(path) = chosen(id) else { continue };
            match &prop.control {
                Control::Folder { mode, .. } => {
                    folders.push((id.clone(), PathBuf::from(path), *mode == wprs_scene::Mode::All))
                }
                Control::File { .. } => files.push((id.clone(), PathBuf::from(path))),
                _ => {}
            }
        }
        let privacy = self.privacy(&pack);
        let storage = (privacy.storage && pack.storage_kib > 0).then(|| self.page_storage(&pack.props.id));
        let storage = storage.filter(|dir| crate::store::private_dir(dir).is_ok());
        let cleared = crate::compositor::lock(&CLEARED).get(&pack.props.id).copied().unwrap_or(0);
        PageSpec { pack, values, privacy, files, folders, storage, cleared }
    }

    pub fn page_on(&self, output: &str, pack: Arc<WebPack>, shown: &mut Option<Arc<Props>>) -> PageSpec {
        let values = pack.props.values(&self.state().config.scene_values(output, &pack.props.id));
        *shown = Some(pack.props.clone());
        self.page_spec(pack, values)
    }

    pub fn exposes(&self, item: &wprs_library::Item) -> Vec<String> {
        let Some(p) = &item.package else { return Vec::new() };
        let network = crate::inputs::hub().is_some_and(|hub| {
            let control = crate::compositor::lock(&hub.control);
            control.grants.get(&p.id).is_some_and(|keys| keys.values().any(|i| i.contains(&Input::Network)))
        });
        wprs_core::model::exposes(&p.kind, &p.facets, &p.confirmed, network)
    }

    pub fn page_storage(&self, id: &str) -> PathBuf {
        self.store.data.join("web").join(id)
    }

    pub fn clear_page_data(&self, id: &str) -> std::io::Result<()> {
        *crate::compositor::lock(&CLEARED).entry(id.to_owned()).or_default() += 1;
        match std::fs::remove_dir_all(self.page_storage(id)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }

    pub fn clear_data(&self, uids: &[&str]) -> crate::error::Result<()> {
        crate::items::limited(uids, crate::items::MAX_UIDS)?;
        let packages = |lib: &wprs_library::Library| {
            uids.iter()
                .map(|uid| Ok(lib.item(uid)?.package.map(|p| p.id)))
                .collect::<wprs_library::Result<Vec<_>>>()
        };
        let ids = self.store.read(packages)?;
        for id in ids.into_iter().flatten() {
            self.clear_page_data(&id)
                .map_err(|e| crate::error::failed(format!("cannot clear the data of {id}: {e}")))?;
        }
        self.moved("hide_storage");
        Ok(())
    }

    pub fn forget_pages(&self, removed: &[wprs_library::Item]) {
        let kept = |id: &str| self.store.read(|lib| lib.copies(id)).is_ok_and(|n| n > 0);
        let web = removed.iter().filter_map(|i| i.package.as_ref()).filter(|p| p.kind == "web");
        for package in web.filter(|p| !kept(&p.id)) {
            if let Err(e) = self.clear_page_data(&package.id) {
                tracing::warn!("cannot remove the data of a web page: {e}");
            }
        }
    }
}
