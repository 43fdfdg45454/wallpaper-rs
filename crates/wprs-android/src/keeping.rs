use crate::pack::Pack;
use serde_json::{Value as Json, json};
use std::collections::BTreeMap;
use std::hash::{BuildHasher, RandomState};
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Instant;
use wprs_core::stash::Keeper;
use wprs_scene::{Control, Mode, Property, Scene, ScriptApi, ScriptOutputs, Values};

const SCREEN: &str = "phone";
const EVERY: f64 = 2.0;
const SHOWN: usize = 8;
static SALT: OnceLock<RandomState> = OnceLock::new();

#[derive(Default)]
pub(crate) struct Keeping {
    keeper: Option<Keeper>,
    pub(crate) text: String,
    files: Vec<(String, PathBuf)>,
    pub(crate) shown: Vec<(String, PathBuf)>,
    listed_at: Option<f64>,
}

fn opaque(parts: impl std::hash::Hash) -> String {
    format!("{:016x}", SALT.get_or_init(RandomState::new).hash_one(parts))
}

pub(crate) fn folders(properties: &BTreeMap<String, Property>) -> Vec<String> {
    let all = |c: &Control| matches!(c, Control::Folder { mode: Mode::All, .. });
    properties.iter().filter(|(_, p)| all(&p.control)).map(|(id, _)| id.clone()).collect()
}

pub(crate) fn forget(lib: &wprs_library::Library, items: &[wprs_library::Item]) -> Result<(), crate::Error> {
    let gone = |p: &&wprs_library::PackageInfo| lib.copies(&p.id).is_ok_and(|n| n == 0);
    for package in items.iter().filter_map(|i| i.package.as_ref()).filter(gone) {
        if package.kind == "web" {
            crate::page::clear(&package.id)?;
        }
        wprs_core::stash::forget(&crate::store::dir()?.join("storage"), &package.id)?;
    }
    Ok(())
}

impl Keeping {
    pub(crate) fn new(pack: &Pack) -> Keeping {
        let stores = pack.scene.scripts.iter().any(|s| s.wants(ScriptApi::Storage));
        let dir = stores.then(|| crate::store::dir().ok()).flatten();
        let keeper = dir.and_then(|d| Keeper::open(&d.join("storage"), &pack.id, SCREEN, true));
        Keeping { keeper, ..Keeping::default() }
    }

    pub(crate) fn scopes(&self) -> Json {
        self.keeper.as_ref().map_or(Json::Null, Keeper::scopes)
    }

    pub(crate) fn storage(&self) -> (u64, &str) {
        self.keeper.as_ref().map_or((0, ""), Keeper::frame)
    }

    pub(crate) fn list(
        &mut self,
        (properties, scene): (&BTreeMap<String, Property>, &Scene),
        values: &Values,
        time: f64,
    ) {
        self.keeper.iter_mut().for_each(|k| k.refresh(Instant::now()));
        let lists = scene.scripts.iter().any(|s| s.wants(ScriptApi::Folders));
        if !lists || self.listed_at.is_some_and(|at| time >= at && time < at + EVERY) {
            return;
        }
        self.listed_at = Some(time);
        let (mut listing, mut files) = (serde_json::Map::new(), Vec::new());
        for id in folders(properties) {
            let dir =
                values.get(&id).and_then(Json::as_str).filter(|v| v.starts_with('/')).map(PathBuf::from);
            let found = dir.map(|d| crate::chosen::listed(&d)).unwrap_or_default();
            let pictures = found.into_iter().filter(|f| crate::chosen::kind(f) == Some("image"));
            let entries: Vec<Json> = pictures
                .map(|path| {
                    let stamp = std::fs::metadata(&path).map_or((0, 0), |m| (m.size(), m.mtime()));
                    let (token, key) = (opaque((&id, &path, stamp)), opaque((&id, &path)));
                    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    files.push((format!("wpak-user://{id}/{token}"), path));
                    json!([token, key, name])
                })
                .collect();
            listing.insert(id, Json::Array(entries));
        }
        (self.text, self.files) = (Json::Object(listing).to_string(), files);
    }

    pub(crate) fn heard(&mut self, out: &mut ScriptOutputs) -> bool {
        if let (Some(keeper), Some(stored)) = (self.keeper.as_mut(), out.storage.take()) {
            keeper.take(&stored, Instant::now());
        }
        let mut asked: Vec<&String> = out.layers.values().filter_map(|l| l.file.as_ref()).collect();
        asked.sort();
        asked.dedup();
        asked.truncate(SHOWN);
        let shown: Vec<(String, PathBuf)> =
            asked.iter().filter_map(|key| self.files.iter().find(|(k, _)| k == *key).cloned()).collect();
        let changed = shown != self.shown;
        self.shown = shown;
        changed
    }
}
