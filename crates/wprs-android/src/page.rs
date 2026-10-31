use crate::pack::Pack;
use crate::power::Power;
use crate::{Error, library, props, settings};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use wprs_core::model::Input;

#[cfg(target_os = "android")]
mod exports;
mod folders;
mod package;
pub(crate) mod script;
pub(crate) mod serve;

pub(crate) use folders::{clear, cleared};
use package::passing;
pub(crate) use package::{Web, host, is_web, pack};

pub(crate) const HIDES: [&str; 4] = ["hide_zone", "hide_language", "hide_gpu", "hide_storage"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Privacy {
    pub(crate) zone: bool,
    pub(crate) language: bool,
    pub(crate) gpu: bool,
    pub(crate) storage: bool,
    pub(crate) network: bool,
}

type Sent = (Map<String, Value>, Value, folders::Folders);

pub(crate) struct View {
    key: String,
    host: String,
    preview: bool,
    pack: Arc<Pack>,
    privacy: Privacy,
    sent: Option<Sent>,
    audio: bool,
    said: u32,
    files: BTreeMap<String, PathBuf>,
}

static VIEWS: Mutex<BTreeMap<i64, View>> = Mutex::new(BTreeMap::new());

fn views() -> MutexGuard<'static, BTreeMap<i64, View>> {
    VIEWS.lock().unwrap_or_else(PoisonError::into_inner)
}

fn decided(id: &str) -> [bool; 4] {
    HIDES.map(|what| library::with(|lib| Ok(lib.decided(id, what)?)).unwrap_or(false))
}

fn privacy(pack: &Pack) -> Privacy {
    let [zone, language, gpu, storage] = decided(&pack.id).map(|hidden| !hidden);
    let web = pack.web.as_ref();
    let network = web.is_some_and(|w| !w.network.is_empty()) && props::allows(&pack.id, Input::Network);
    let storage = storage && web.is_some_and(|w| w.storage);
    Privacy { zone, language, gpu, storage, network }
}

fn settings(pack: &Pack, preview: bool) -> Value {
    let power = Power::now();
    let languages = library::languages();
    let locale = languages.first().cloned().or_else(|| pack.locale.clone()).unwrap_or_default();
    let muted = settings::setting("playback.muted", None).map_or(true, |m| m != "false");
    let volume = if muted { 0.0 } else { 1.0 };
    json!({"fps": power.fps(preview), "quality": power.ceiling() as u8, "locale": locale, "volume": volume})
}

pub(crate) fn spec(handle: i64, uid: &str, path: &Path, preview: bool) -> Result<Value, Error> {
    let pack = crate::pack::load(path)?;
    let Some(web) = pack.web.as_ref() else { return Ok(Value::Null) };
    let privacy = privacy(&pack);
    let stamp = std::fs::metadata(path).map(|m| (m.len(), m.modified().ok())).ok();
    let generation = folders::count(&pack.id);
    let key = format!("{uid}:{privacy:?}:{generation}:{stamp:?}");
    let fresh = || View {
        key: key.clone(),
        host: if privacy.storage { host(&pack.id, generation) } else { passing() },
        preview,
        pack: pack.clone(),
        privacy,
        sent: None,
        audio: false,
        said: 0,
        files: BTreeMap::new(),
    };
    let mut views = views();
    let view = views.entry(handle).or_insert_with(fresh);
    if view.key != key {
        *view = fresh();
    }
    view.pack = pack.clone();
    let tell = script::changes(view);
    let ([audio, pointer, clicks], granted) = (web.uses, props::granted(&pack.id));
    Ok(json!({
        "key": key, "url": format!("https://{}/", view.host), "origin": format!("https://{}", view.host),
        "background": web.background, "network": privacy.network, "storage": privacy.storage,
        "fps": Power::now().fps(preview), "audio": audio && granted[0] && view.audio,
        "pointer": pointer && granted[1], "clicks": clicks && granted[2], "tell": tell,
    }))
}

pub(crate) fn close(handle: i64) {
    views().remove(&handle);
}

pub(crate) fn hearing(handle: i64) -> bool {
    let views = views();
    let Some(v) = views.get(&handle) else { return false };
    v.audio && v.pack.web.as_ref().is_some_and(|w| w.uses[0]) && props::granted(&v.pack.id)[0]
}

pub(crate) fn touched(handle: i64) -> bool {
    let views = views();
    let Some(v) = views.get(&handle) else { return false };
    let granted = props::granted(&v.pack.id);
    v.pack.web.as_ref().is_some_and(|w| (w.uses[1] && granted[1]) || (w.uses[2] && granted[2]))
}

pub(crate) fn exposes(package: &wprs_library::PackageInfo) -> Vec<String> {
    let network = props::allows(&package.id, Input::Network);
    wprs_core::model::exposes(&package.kind, &package.facets, &package.confirmed, network)
}

pub(crate) fn form(pack: &Pack) -> Value {
    let Some(web) = pack.web.as_ref() else { return Value::Null };
    let hidden = decided(&pack.id);
    let confirmed: Vec<String> =
        HIDES.iter().zip(hidden).filter(|(_, h)| *h).map(|(w, _)| w.to_string()).collect();
    let facets = [(web.storage, "storage"), (!web.network.is_empty(), "network")];
    let facets: Vec<String> = facets.into_iter().filter(|f| f.0).map(|f| f.1.to_owned()).collect();
    let online = props::allows(&pack.id, Input::Network);
    json!({
        "exposes": wprs_core::model::exposes("web", &facets, &confirmed, online),
        "hidden": {"zone": hidden[0], "language": hidden[1], "gpu": hidden[2], "storage": hidden[3]},
        "storage": web.storage, "network": !web.network.is_empty(), "online": online,
        "origin": format!("https://{}", host(&pack.id, folders::count(&pack.id))),
    })
}
