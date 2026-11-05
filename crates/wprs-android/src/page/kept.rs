use super::View;
use crate::{Error, store};
use serde_json::{Map, Value};
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};

const FULL: &str = "web-full.json";
const KEPT: &str = include_str!("kept.js");

static HOSTS: Mutex<Option<BTreeSet<String>>> = Mutex::new(None);

fn file() -> Result<PathBuf, Error> {
    Ok(store::dir()?.join(FULL))
}

fn with<T>(work: impl FnOnce(&mut BTreeSet<String>) -> T) -> T {
    let mut hosts = HOSTS.lock().unwrap_or_else(PoisonError::into_inner);
    let hosts = hosts.get_or_insert_with(|| {
        let read = file().ok().and_then(|path| fs::read(path).ok());
        read.and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default()
    });
    work(hosts)
}

pub(crate) fn full(host: &str) -> bool {
    with(|hosts| hosts.contains(host))
}

pub(crate) fn forget(hosts: &[String]) -> Result<(), Error> {
    with(|known| {
        let before = known.len();
        known.retain(|h| !hosts.iter().any(|gone| gone.trim_start_matches("https://") == h));
        if known.len() == before { Ok(()) } else { store::write(&file()?, &serde_json::to_vec(known)?) }
    })
}

pub(super) fn allowance(view: &View) -> Option<u64> {
    let kib = view.pack.web.as_ref().map_or(0, |w| w.storage_kib);
    (view.privacy.storage && kib > 0).then(|| wprs_format::kept_allowance(kib))
}

pub(crate) fn capped(allowed: u64, post: &str) -> String {
    format!("({})({allowed}, {post});", KEPT.trim_end())
}

pub(super) fn script(view: &View, post: &str) -> Option<String> {
    allowance(view).map(|allowed| capped(allowed, post))
}

pub(super) fn measured(view: &mut View, message: &Map<String, Value>) -> bool {
    let bytes = |key: &str| message.get(key).and_then(Value::as_u64);
    let (Some(allowed), Some(at), Some(bytes)) = (
        allowance(view),
        ["kept", "used"].iter().position(|k| message.contains_key(*k)),
        bytes("kept").or_else(|| bytes("used")),
    ) else {
        return false;
    };
    view.kept[at] = bytes;
    if view.kept.iter().sum::<u64>() <= allowed {
        return false;
    }
    let host = view.host.clone();
    let marked = with(|hosts| {
        hosts.insert(host);
        file().and_then(|path| store::write(&path, &serde_json::to_vec(hosts)?))
    });
    if let Err(e) = marked {
        log::warn!("cannot keep that a page is full: {e}");
    }
    store::changed();
    true
}
