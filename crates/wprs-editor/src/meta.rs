use crate::check::Problem;
use crate::ops::{Op, apply, diff, set};
use anyhow::Result;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::Path;
use wprs_format::features::{check, facts, lacks, playable, warnings};
use wprs_format::{
    Content, Descriptor, Error, Facts, Input, Manifest, Origin, Platform, Rating, Requires, Scene, Text,
    newer_names,
};

const TOP: [&str; 10] = [
    "schema",
    "kind",
    "entry",
    "properties",
    "strings",
    "preview",
    "playback",
    "web",
    "permissions",
    "presets",
];
const FIRST: [&str; 8] = ["title", "description", "version", "id", "authors", "license", "preview", "tags"];
pub const ORDER: &str = "title description changes version id authors license content rating descriptors \
    photosensitive origin kind derived_from credits preview previews thumbnail animated links homepage source \
    support donate tags inputs platforms display accent requires features engine packages default_locale presets";

pub fn hidden(doc: &Value) -> Vec<&'static str> {
    if doc["manifest"]["schema"] != wprs_format::SCHEMA_V1 {
        return TOP.to_vec();
    }
    let all =
        crate::schema::MANIFEST["properties"].as_object().into_iter().flatten().map(|(k, _)| k.as_str());
    all.filter(|k| !FIRST.contains(k)).collect()
}

pub fn applies(doc: &Value, pointer: &str) -> bool {
    let framing: &[&str] = match doc["manifest"]["kind"].as_str() {
        Some("scene") => &["fit", "zoom", "position", "background", "flip"],
        Some("image") => &["zoom", "position"],
        _ => &[],
    };
    let key = pointer.strip_prefix("/manifest/display/").and_then(|k| k.split('/').next());
    key.is_none_or(|k| !framing.contains(&k)) || doc.pointer(pointer).is_some()
}

pub fn adjust(doc: &Value, field: &mut crate::Field) {
    match field.pointer.as_str() {
        "/manifest/inputs" => field.fixed = used(doc),
        "/manifest/default_locale" => field.required = true,
        "/manifest/requires/features" => {
            field.options = wprs_format::features::CHOSEN.map(Value::from).to_vec()
        }
        _ => crate::settings::adjust(doc, field),
    }
}

pub fn facts_of(doc: &Value, dir: Option<&Path>) -> Option<(Manifest, Facts)> {
    let manifest = read(doc, dir).ok()?;
    let found = found(doc, &manifest, &doc["scene"], dir)?;
    Some((manifest, found))
}

pub fn read(doc: &Value, dir: Option<&Path>) -> wprs_format::Result<Manifest> {
    let manifest = Manifest::deserialize(&doc["manifest"])?;
    dir.map_or(Ok(manifest.clone()), |dir| crate::cache::with_strings(dir, manifest))
}

fn found(doc: &Value, manifest: &Manifest, json: &Value, dir: Option<&Path>) -> Option<Facts> {
    let mut scene = Scene::deserialize(json).ok()?;
    if let Some(dir) = dir {
        crate::cache::read_files(&mut scene, dir);
    }
    let mut found = facts(manifest, Some(&scene), None);
    let newer = newer_names(&doc["manifest"], || wprs_scene::newer_scene_keys(json), &found.features);
    found.features.extend(newer);
    Some(found)
}

pub fn detected(doc: &Value, dir: Option<&Path>) -> BTreeSet<Input> {
    let declared = doc["manifest"]["inputs"].as_array().into_iter().flatten();
    let detectable = declared.filter_map(Value::as_str).any(|i| ["audio", "pointer", "clicks"].contains(&i));
    detectable.then(|| facts_of(doc, dir).map(|(_, f)| f.inputs)).flatten().unwrap_or_default()
}

pub fn sync(doc: &Value, before: &BTreeSet<Input>, dir: Option<&Path>) -> Vec<Op> {
    synced(doc, &doc["scene"], before, dir)
}

fn synced(doc: &Value, scene: &Value, before: &BTreeSet<Input>, dir: Option<&Path>) -> Vec<Op> {
    let manifest = read(doc, dir).ok().filter(Manifest::v2);
    let Some(mut manifest) = manifest else { return Vec::new() };
    let Some(found) = found(doc, &manifest, scene, dir) else { return Vec::new() };
    let gone = |i: &Input| before.contains(i) && !found.inputs.contains(i);
    let dropped: Vec<String> =
        manifest.inputs.iter().filter(|i| gone(i)).map(|i| format!("inputs.{}", i.name())).collect();
    manifest.inputs.retain(|i| !gone(i));
    manifest.inputs.extend(found.inputs.iter().copied());
    manifest.inputs.sort();
    manifest.inputs.dedup();
    let features: Vec<String> = found.features.into_iter().filter(|f| !dropped.contains(f)).collect();
    let added = doc["manifest"]["requires"] == json!({});
    let old = Requires { features: Vec::new(), ..manifest.requires.take().unwrap_or_default() };
    let kept = added || old != Requires::default() || !features.is_empty();
    manifest.requires = kept.then_some(Requires { features, ..old });
    let want = serde_json::to_value(&manifest).unwrap_or_default();
    let (mut ops, path) = (Vec::new(), |key: &str| format!("/manifest/{key}"));
    for key in ["inputs", "requires"] {
        match (doc["manifest"].get(key), want.get(key)) {
            (now, Some(next)) if now != Some(next) => ops.extend(set(doc, &path(key), next.clone())),
            (Some(_), None) => ops.push(Op::Remove { path: path(key) }),
            _ => {}
        }
    }
    ops
}

pub fn cancels(doc: &Value, ops: &[Op], dir: Option<&Path>) -> bool {
    let computed = |p: &str| ["/manifest/inputs", "/manifest/requires"].iter().any(|s| p.starts_with(s));
    if !ops.iter().flat_map(Op::paths).all(computed) {
        return false;
    }
    let mut probe = json!({"manifest": doc["manifest"].clone()});
    if apply(&mut probe, ops).is_err() {
        return false;
    }
    let follow = synced(&probe, &doc["scene"], &BTreeSet::new(), dir);
    apply(&mut probe, &follow).is_ok() && probe["manifest"] == doc["manifest"]
}

fn at(field: &str) -> String {
    match field {
        f if f.starts_with("inputs") => "/manifest/inputs",
        f if f.starts_with("the scene costs") => "/scene/layers",
        _ => "/manifest/requires/features",
    }
    .into()
}

pub fn problems(doc: &Value, dir: Option<&Path>) -> Vec<Problem> {
    let Some((manifest, found)) = facts_of(doc, dir).filter(|(m, _)| m.v2()) else { return Vec::new() };
    let mut out: Vec<Problem> = check(&manifest, &found)
        .err()
        .into_iter()
        .map(|e| match e {
            Error::Invalid { field, reason } => Problem {
                pointer: at(&field),
                message: format!("{field}: {reason}"),
                warning: field != "inputs",
            },
            e => Problem { pointer: "/manifest".into(), message: e.to_string(), warning: false },
        })
        .collect();
    let unread = dir.map(|d| manifest.unread_texts(wprs_format::locale_names(d))).unwrap_or_default();
    let notes = warnings(&manifest, &found).into_iter().chain(unread);
    out.extend(notes.map(|message| Problem { pointer: at(&message), message, warning: true }));
    out.extend(verdicts(&manifest, &found).into_iter().map(|message| Problem {
        pointer: "/manifest/requires/features".into(),
        message,
        warning: true,
    }));
    out
}

pub fn verdicts(manifest: &Manifest, found: &Facts) -> Vec<String> {
    let verdict = |p: Platform| match (playable(manifest, found, p), lacks(p.name(), &found.features)) {
        (Err(e), _) => Some(format!("{}: refused: {e}", p.name())),
        (Ok(()), without) if !without.is_empty() => {
            Some(format!("{}: plays without {}", p.name(), without.join(", ")))
        }
        _ => None,
    };
    Platform::ALL.into_iter().filter_map(verdict).collect()
}

pub fn used(doc: &Value) -> Vec<Value> {
    let found = facts_of(doc, None).map(|(_, f)| f.inputs).unwrap_or_default();
    found.into_iter().map(|i| json!(i.name())).collect()
}

pub fn title(doc: &Value, languages: &[String]) -> String {
    match Manifest::deserialize(&doc["manifest"]) {
        Ok(manifest) => manifest.title_in(languages),
        Err(_) => Text::deserialize(&doc["manifest"]["title"]).map(|t| t.pick(languages)).unwrap_or_default(),
    }
}

pub fn languages(doc: &Value) -> Vec<Value> {
    let current = doc["manifest"]["default_locale"].as_str().map(String::from);
    let used = Manifest::deserialize(&doc["manifest"]).map(|m| m.locales()).unwrap_or_default();
    let mut out: Vec<String> = Vec::new();
    for language in current.into_iter().chain(used).chain([crate::starters::locale()]) {
        if !out.iter().any(|o| wprs_scene::tag(o) == wprs_scene::tag(&language)) {
            out.push(language);
        }
    }
    out.sort();
    out.into_iter().map(Value::from).collect()
}

pub fn upgrade(
    doc: &Value,
    rating: Option<Rating>,
    reasons: &[Descriptor],
    photosensitive: Option<bool>,
    derived_from: &[String],
) -> Result<Vec<Op>> {
    let (mut manifest, mut scene) = (doc["manifest"].clone(), doc["scene"].clone());
    wprs_format::upgrade_json(&mut manifest, Some(&mut scene))?;
    if let Some(rating) = rating {
        manifest["content"] = serde_json::to_value(Content { rating, descriptors: reasons.to_vec() })?;
    }
    if let Some(photosensitive) = photosensitive {
        manifest["photosensitive"] = json!(photosensitive);
    }
    if rating.is_some() || !derived_from.is_empty() {
        manifest["origin"] = serde_json::to_value(Origin::by(derived_from))?;
    }
    let mut ops = diff(&doc["manifest"], &manifest, "/manifest");
    ops.extend(diff(&doc["scene"], &scene, "/scene"));
    Ok(ops)
}
