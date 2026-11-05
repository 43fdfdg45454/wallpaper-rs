use crate::pack::Pack;
use crate::power::Power;
use crate::{Error, library, props, settings};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use wprs_core::model::Input;

#[cfg(target_os = "android")]
mod exports;
mod folders;
mod heard;
pub(crate) mod kept;
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
    heard: Option<(bool, u64, u64)>,
    kept: [u64; 2],
}

static VIEWS: Mutex<BTreeMap<i64, View>> = Mutex::new(BTreeMap::new());

fn views() -> MutexGuard<'static, BTreeMap<i64, View>> {
    VIEWS.lock().unwrap_or_else(PoisonError::into_inner)
}

fn decided(id: &str) -> [bool; 4] {
    HIDES.map(|what| library::with(|lib| Ok(lib.decided(id, what)?)).unwrap_or(false))
}

pub(crate) fn privacy(pack: &Pack) -> Privacy {
    let [zone, language, gpu, storage] = decided(&pack.id).map(|hidden| !hidden);
    let web = pack.web.as_ref();
    let network = web.is_some_and(|w| !w.network.is_empty()) && props::allows(&pack.id, Input::Network);
    let storage = storage
        && web.is_some_and(|w| w.storage_kib > 0)
        && !kept::full(&host(&pack.id, folders::count(&pack.id)));
    Privacy { zone, language, gpu, storage, network }
}

pub(crate) const PLAIN: &str = "en-US";

fn languages(pack: &Pack, privacy: Privacy) -> Vec<String> {
    let user = if privacy.language { library::languages() } else { vec![PLAIN.into()] };
    user.into_iter().chain(pack.locale.clone()).collect()
}

static DUCK: AtomicU32 = AtomicU32::new(0x3f80_0000);

pub(crate) fn duck(gain: f32) {
    let gain = if gain.is_finite() { gain.clamp(0.0, 1.0) } else { 1.0 };
    DUCK.store(gain.to_bits(), Relaxed);
}

pub(crate) fn settings(pack: &Pack, preview: bool, privacy: Privacy) -> Value {
    let power = Power::now();
    let locale = languages(pack, privacy).first().cloned().unwrap_or_default();
    let muted = settings::setting("playback.muted", None).map_or(true, |m| m != "false");
    let added = pack.web.as_ref().filter(|w| !w.own_volume).map(|_| props::values(pack));
    let own =
        added.and_then(|v| v.get(wprs_scene::VOLUME)?.as_f64()).map_or(1.0, |v| v.clamp(0.0, 100.0) / 100.0);
    let volume = if muted { 0.0 } else { f64::from(f32::from_bits(DUCK.load(Relaxed))) * own };
    json!({"fps": fps(pack, preview), "quality": power.ceiling() as u8, "locale": locale, "volume": volume})
}

pub(crate) fn fps(pack: &Pack, preview: bool) -> u32 {
    let cap = pack.web.as_ref().map_or(wprs_core::settings::fps::DISPLAY, |w| w.fps);
    wprs_core::settings::fps::lower(Power::now().fps(preview), cap)
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
        heard: None,
        kept: [0; 2],
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
        "fps": fps(&pack, preview), "audio": audio && granted[0] && view.audio,
        "pointer": pointer && granted[1], "clicks": clicks && granted[2], "tell": tell,
        "guard": script::started(view),
    }))
}

pub(crate) fn close(handle: i64) {
    views().remove(&handle);
    crate::opened::forget(handle);
}

pub(crate) fn heeds(handle: i64) -> bool {
    views().get(&handle).is_some_and(|v| v.heard.is_some() && heard::allowed(v))
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
    let facets = [(web.storage_kib > 0, "storage"), (!web.network.is_empty(), "network")];
    let facets: Vec<String> = facets.into_iter().filter(|f| f.0).map(|f| f.1.to_owned()).collect();
    let online = props::allows(&pack.id, Input::Network);
    let origin = host(&pack.id, folders::count(&pack.id));
    json!({
        "exposes": wprs_core::model::exposes("web", &facets, &confirmed, online),
        "hidden": {"zone": hidden[0], "language": hidden[1], "gpu": hidden[2], "storage": hidden[3]},
        "storage": web.storage_kib > 0, "network": !web.network.is_empty(), "online": online, "own_volume": web.own_volume,
        "origin": format!("https://{origin}"), "full": kept::full(&origin),
    })
}
