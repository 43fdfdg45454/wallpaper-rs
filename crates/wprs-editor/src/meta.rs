use crate::check::Problem;
use crate::ops::{Op, apply, diff, set};
use anyhow::Result;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use wprs_format::features::{check, facts, warnings};
use wprs_format::{
    Content, Descriptor, Error, Facts, Input, Manifest, Origin, Rating, Requires, Scene, Text,
};

const TOP: [&str; 6] = ["schema", "kind", "entry", "properties", "strings", "preview"];
const FIRST: [&str; 8] = ["title", "description", "version", "id", "authors", "license", "preview", "tags"];
pub const ORDER: &str = "title description changes version id authors license content rating descriptors \
    photosensitive origin kind derived_from credits preview previews thumbnail animated links homepage source \
    support donate tags inputs requires features engine default_locale presets";

pub fn hidden(doc: &Value) -> Vec<&'static str> {
    if doc["manifest"]["schema"] != wprs_format::SCHEMA_V1 {
        return TOP.to_vec();
    }
    let all =
        crate::schema::MANIFEST["properties"].as_object().into_iter().flatten().map(|(k, _)| k.as_str());
    all.filter(|k| !FIRST.contains(k)).collect()
}

pub fn adjust(doc: &Value, field: &mut crate::Field) {
    match field.pointer.as_str() {
        "/manifest/inputs" => field.fixed = used(doc),
        "/manifest/default_locale" => field.required = true,
        _ => {}
    }
}

pub fn facts_of(doc: &Value) -> Option<(Manifest, Facts)> {
    let manifest = Manifest::deserialize(&doc["manifest"]).ok()?;
    let scene = Scene::deserialize(&doc["scene"]).ok()?;
    let found = facts(&manifest, Some(&scene), None);
    Some((manifest, found))
}

pub fn detected(doc: &Value) -> BTreeSet<Input> {
    let declared = doc["manifest"]["inputs"].as_array().into_iter().flatten();
    let detectable = declared.filter_map(Value::as_str).any(|i| ["audio", "pointer", "clicks"].contains(&i));
    detectable.then(|| facts_of(doc).map(|(_, f)| f.inputs)).flatten().unwrap_or_default()
}

pub fn sync(doc: &Value, before: &BTreeSet<Input>) -> Vec<Op> {
    synced(doc, &doc["scene"], before)
}

fn synced(doc: &Value, scene: &Value, before: &BTreeSet<Input>) -> Vec<Op> {
    let manifest = Manifest::deserialize(&doc["manifest"]).ok().filter(Manifest::v2);
    let (Some(mut manifest), Ok(scene)) = (manifest, Scene::deserialize(scene)) else { return Vec::new() };
    let found = facts(&manifest, Some(&scene), None);
    let gone = |i: &Input| before.contains(i) && !found.inputs.contains(i);
    let dropped: Vec<String> =
        manifest.inputs.iter().filter(|i| gone(i)).map(|i| format!("inputs.{}", i.name())).collect();
    manifest.inputs.retain(|i| !gone(i));
    manifest.inputs.extend(found.inputs.iter().copied());
    manifest.inputs.sort();
    manifest.inputs.dedup();
    let features: Vec<String> = found.features.into_iter().filter(|f| !dropped.contains(f)).collect();
    let added = doc["manifest"]["requires"] == json!({});
    let engine = manifest.requires.take().and_then(|r| r.engine);
    let kept = added || engine.is_some() || !features.is_empty();
    manifest.requires = kept.then_some(Requires { engine, features });
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

pub fn cancels(doc: &Value, ops: &[Op]) -> bool {
    let computed = |p: &str| ["/manifest/inputs", "/manifest/requires"].iter().any(|s| p.starts_with(s));
    if !ops.iter().flat_map(Op::paths).all(computed) {
        return false;
    }
    let mut probe = json!({"manifest": doc["manifest"].clone()});
    if apply(&mut probe, ops).is_err() {
        return false;
    }
    let follow = synced(&probe, &doc["scene"], &BTreeSet::new());
    apply(&mut probe, &follow).is_ok() && probe["manifest"] == doc["manifest"]
}

fn at(field: &str) -> String {
    if field.starts_with("inputs") { "/manifest/inputs" } else { "/manifest/requires/features" }.into()
}

pub fn problems(doc: &Value) -> Vec<Problem> {
    let Some((manifest, found)) = facts_of(doc).filter(|(m, _)| m.v2()) else { return Vec::new() };
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
    let notes = warnings(&manifest, &found).into_iter();
    out.extend(notes.map(|message| Problem { pointer: at(&message), message, warning: true }));
    out
}

pub fn used(doc: &Value) -> Vec<Value> {
    let found = facts_of(doc).map(|(_, f)| f.inputs).unwrap_or_default();
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
