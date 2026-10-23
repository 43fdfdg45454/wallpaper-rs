use crate::{Doc, Hlc, Kept, Local, Replica, Rules, View};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub fn rules(collection: &str) -> Rules {
    match collection {
        "library" => {
            Rules { weak: |key| key == "title", keep_absent: true, needs_strong: true, ..Rules::default() }
        }
        "settings" => Rules { weak: |key| key.contains('.'), ..Rules::default() },
        "playlists" => Rules { clash: crate::clash, ..Rules::default() },
        "repos" => Rules::default(),
        _ => Rules { keep_absent: true, ..Rules::default() },
    }
}

pub fn arrived(
    local: &Local,
    replica: &Replica,
    applied: &[(String, Option<View>)],
    start: &BTreeMap<String, (i64, Doc)>,
) -> Vec<(String, Option<View>)> {
    let secret = |key: &str| {
        key.starts_with("secrets/") && start.get(key).map(|b| b.0) != replica.bases.get(key).map(|b| b.0)
    };
    let mentions =
        |view: &View| view.values().any(|v| local.fresh.iter().any(|k| v.to_string().contains(k.as_str())));
    let fresh = |key: &str| key.strip_prefix("library/").is_some_and(|k| local.fresh.contains(k));
    let views = replica.bases.iter().filter(|(key, _)| !applied.iter().any(|(k, _)| k == *key));
    let views = views.filter_map(|(key, (_, doc))| Some((key, doc.view()?)));
    let wanted =
        |key: &str, view: &View| fresh(key) || secret(key) || local.unlinked.contains(key) || mentions(view);
    views.filter(|(key, view)| wanted(key, view)).map(|(key, view)| (key.clone(), Some(view))).collect()
}

pub fn hold(
    replica: &mut Replica,
    before: &BTreeMap<String, (i64, Doc)>,
    local: &Local,
    failed: &[(String, String)],
    now: &Hlc,
) -> BTreeMap<String, i64> {
    let mut held = BTreeMap::new();
    for (key, _) in failed {
        let Some(after) = replica.bases.get(key).cloned() else { continue };
        let old = before.get(key);
        let weak = rules(key.split('/').next().unwrap_or_default()).weak;
        let shown = Doc::stamp(old.map(|(_, doc)| doc), local.views.get(key), weak, now);
        held.insert(key.clone(), after.0);
        replica.held.insert(key.clone(), after);
        match shown.or_else(|| old.map(|(_, doc)| doc.clone())) {
            Some(doc) => drop(replica.bases.insert(key.clone(), (old.map_or(0, |(seq, _)| *seq), doc))),
            None => drop(replica.bases.remove(key)),
        }
    }
    held
}

pub fn keep(kept: &mut Kept, key: &str, held: BTreeMap<String, Value>) {
    match held.is_empty() {
        true => drop(kept.remove(key)),
        false => drop(kept.insert(key.to_owned(), held)),
    }
}

pub fn text(view: &View, field: &str, fallback: &str) -> String {
    view.get(field).and_then(Value::as_str).unwrap_or(fallback).to_owned()
}

pub fn playlist_rules(
    view: &View,
    current: &wprs_library::Rules,
) -> (wprs_library::Rules, BTreeMap<String, Value>) {
    let shown = serde_json::to_value(current).unwrap_or_default();
    let (mut merged, mut held) =
        (serde_json::to_value(wprs_library::Rules::default()).unwrap_or_default(), BTreeMap::new());
    for (field, value) in view.iter().filter_map(|(k, v)| Some((k.strip_prefix("r:")?, v))) {
        if merged.get(field).is_none() {
            continue;
        }
        let mut tried = merged.clone();
        tried[field] = value.clone();
        if serde_json::from_value::<wprs_library::Rules>(tried.clone()).is_ok() {
            merged = tried;
        } else {
            let old = shown.get(field).cloned().unwrap_or_default();
            merged[field] = old.clone();
            held.insert(format!("r:{field}"), old);
        }
    }
    (serde_json::from_value(merged).unwrap_or_else(|_| current.clone()), held)
}

pub fn keep_name(kept: &mut Kept, key: &str, shown: &str, received: &str) {
    let fields = kept.entry(key.to_owned()).or_default();
    match shown == received {
        true => drop(fields.remove("name")),
        false => drop(fields.insert("name".into(), json!(shown))),
    }
    if fields.is_empty() {
        kept.remove(key);
    }
}

pub fn renamed<T>(
    name: &str,
    mut create: impl FnMut(&str) -> wprs_library::Result<T>,
) -> wprs_library::Result<T> {
    match create(name) {
        Err(wprs_library::Error::Exists(..)) => create(&format!("{name} (2)")),
        done => done,
    }
}
