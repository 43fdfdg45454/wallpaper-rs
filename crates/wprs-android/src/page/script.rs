use super::folders::{self, Folders};
use super::{View, views};
use crate::library;
use serde_json::{Value, json};
use std::hash::{BuildHasher, RandomState};
use wprs_format::page::{MASK, api};

const POST: &str = "((h) => (m) => h.post(JSON.stringify(m)))(window.wprsHost)";
const PHONE: &str = include_str!("phone.js");
const HIDE: &str = include_str!("hide.js");
const MAX_SAID: u32 = 200;
const BANDS: usize = 64;

fn call(messages: &[Value]) -> Option<String> {
    let calls: Vec<String> = messages.iter().map(|m| format!("window.__wprs({m})")).collect();
    (!calls.is_empty()).then(|| format!("window.__wprs && ({})", calls.join(", ")))
}

pub(super) fn source(view: &mut View) -> String {
    let (values, folders) = folders::values(view);
    let settings = super::settings(&view.pack, view.preview);
    let languages: Vec<String> = library::languages().into_iter().chain(view.pack.locale.clone()).collect();
    let kinds: serde_json::Map<String, Value> =
        view.pack.properties.iter().map(|(id, p)| (id.clone(), json!(p.kind()))).collect();
    let init = json!({
        "properties": values, "kinds": kinds, "settings": settings,
        "labels": wprs_scene::labels(&view.pack.properties, &languages),
    });
    view.sent = Some((values, settings, folders));
    let p = view.privacy;
    let hide =
        (!p.zone || !p.language).then(|| format!("({})({}, {});", HIDE.trim_end(), !p.zone, !p.language));
    let mask = (!p.gpu).then_some(MASK);
    let preload = view.pack.web.as_ref().and_then(|w| w.preload.clone());
    let parts = [Some(api(&init, POST)), hide, mask.map(String::from), Some(PHONE.into()), preload];
    parts.into_iter().flatten().collect::<Vec<_>>().join("\n")
}

pub(super) fn changes(view: &mut View) -> Option<String> {
    let (old_values, old_settings, old_folders) = view.sent.clone()?;
    let (values, folders) = folders::values(view);
    let settings = super::settings(&view.pack, view.preview);
    let mut messages = Vec::new();
    let changed: serde_json::Map<String, Value> = values
        .iter()
        .filter(|(k, v)| old_values.get(*k) != Some(v))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    if !changed.is_empty() {
        messages.push(json!({"properties": changed}));
    }
    if settings != old_settings {
        messages.push(json!({"settings": settings}));
    }
    for (name, now) in folders.iter().filter(|(name, _)| name.starts_with('*')) {
        let before = old_folders.get(name).cloned().unwrap_or_default();
        let added: Vec<&String> = now.iter().filter(|t| !before.contains(t)).collect();
        let removed: Vec<&String> = before.iter().filter(|t| !now.contains(t)).collect();
        if !added.is_empty() || !removed.is_empty() {
            messages.push(json!({"files": {"prop": &name[1..], "added": added, "removed": removed}}));
        }
    }
    view.sent = Some((values, settings, folders));
    call(&messages)
}

fn random(list: &[String]) -> Value {
    match list.len() {
        0 => Value::Null,
        n => json!(list[RandomState::new().hash_one(n) as usize % n]),
    }
}

fn reply(id: &Value, value: Value) -> Option<String> {
    call(&[json!({"reply": {"id": id, "value": value}})])
}

pub(crate) fn heard(handle: i64, text: &str) -> Option<String> {
    let Ok(Value::Object(message)) = serde_json::from_str::<Value>(text) else { return None };
    let mut views = views();
    let view = views.get_mut(&handle)?;
    let (id, empty) = (message.get("id").cloned().unwrap_or(Value::Null), Folders::new());
    let folders = view.sent.as_ref().map_or(&empty, |s| &s.2);
    if let Some(prop) = message.get("files").and_then(Value::as_str) {
        return reply(&id, folders::listed(folders, prop, true).map_or(Value::Null, |t| json!(t)));
    }
    if let Some(prop) = message.get("random").and_then(Value::as_str) {
        return reply(&id, random(folders::listed(folders, prop, false).map_or(&[][..], Vec::as_slice)));
    }
    if message.get("audio").is_some() {
        view.audio = true;
        crate::store::changed();
    }
    if let Some(said) = message.get("error").and_then(Value::as_str) {
        view.said += 1;
        if view.said <= MAX_SAID {
            log::info!("page: {}", said.chars().take(512).collect::<String>());
        }
    }
    None
}

pub(crate) fn listen(handle: i64) -> Option<String> {
    super::hearing(handle).then_some(())?;
    let frame = crate::hearing::frame();
    let audio = [frame.left[..BANDS].to_vec(), frame.right[..BANDS].to_vec()];
    call(&[json!({"audio": audio})])
}
