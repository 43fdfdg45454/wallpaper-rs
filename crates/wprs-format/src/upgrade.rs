use crate::meta::{Content, Origin};
use crate::names::valid_property;
use crate::source::facts_of;
use crate::{Kind, MANIFEST, Manifest, Result, SCHEMA, SCHEMA_V1, SIGNATURE, Text, check_path, require};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use wprs_scene::tag;

pub type Renames = BTreeMap<String, String>;

pub fn upgrade_json(manifest: &mut Value, scene: Option<&mut Value>) -> Result<Renames> {
    require(manifest["schema"] != SCHEMA, "schema", "is already wallpaper-rs.wpak/2")?;
    require(manifest["schema"] == SCHEMA_V1, "schema", "expected wallpaper-rs.wpak/1")?;
    manifest["schema"] = json!(SCHEMA);
    if let Some(preview) = manifest.as_object_mut().and_then(|m| m.remove("preview")) {
        manifest["previews"] = json!({"thumbnail": preview});
    }
    let renames = renames(manifest);
    rename_json(manifest, scene, &renames);
    if manifest.get("default_locale").is_none()
        && let Some(locale) = common(manifest)
    {
        manifest["default_locale"] = json!(locale);
    }
    Ok(renames)
}

pub fn rename_json(manifest: &mut Value, scene: Option<&mut Value>, renames: &Renames) {
    rekey(manifest.get_mut("properties"), renames);
    let props = manifest.get_mut("properties").and_then(Value::as_object_mut);
    for prop in props.into_iter().flat_map(|p| p.values_mut()) {
        if let Some(group) = prop.get("group").and_then(Value::as_str).and_then(|g| renames.get(g)) {
            prop["group"] = json!(group);
        }
        for key in ["visible_if", "enabled_if"] {
            match prop.get_mut(key) {
                Some(Value::String(text)) => {
                    *text =
                        renames.iter().fold(text.clone(), |t, (from, to)| wprs_scene::rename(&t, from, to))
                }
                found => rekey(found, renames),
            }
        }
    }
    for preset in manifest.get_mut("presets").and_then(Value::as_array_mut).into_iter().flatten() {
        rekey(preset.get_mut("values"), renames);
    }
    scene.into_iter().for_each(|scene| bind(scene, renames));
}

fn renames(manifest: &Value) -> Renames {
    let ids: Vec<&String> =
        manifest["properties"].as_object().map(|p| p.keys().collect()).unwrap_or_default();
    let mut taken: BTreeSet<String> =
        ids.iter().filter(|id| valid_property(id)).map(|id| id.to_lowercase()).collect();
    let mut out = Renames::new();
    for id in ids.into_iter().filter(|id| !valid_property(id)) {
        let base: String =
            id.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).take(56).collect();
        let base =
            if base.starts_with(|c: char| c.is_ascii_alphabetic()) { base } else { format!("p_{base}") };
        let named = |n: usize| if n < 2 { base.clone() } else { format!("{base}_{n}") };
        let name = (1..).map(named).find(|n| !taken.contains(&n.to_lowercase())).unwrap_or_default();
        taken.insert(name.to_lowercase());
        out.insert(id.clone(), name);
    }
    out
}

fn rekey(value: Option<&mut Value>, renames: &Renames) {
    let Some(map) = value.and_then(Value::as_object_mut) else { return };
    *map = std::mem::take(map).into_iter().map(|(k, v)| (renames.get(&k).cloned().unwrap_or(k), v)).collect();
}

fn bind(value: &mut Value, renames: &Renames) {
    match value {
        Value::Object(map) => map.iter_mut().for_each(|(key, v)| {
            let next = match (key.as_str(), v.as_str()) {
                ("prop", Some(id)) => renames.get(id).map(|n| json!(n)),
                ("trigger", Some(t)) => {
                    t.strip_prefix("prop:").and_then(|id| renames.get(id)).map(|n| json!(format!("prop:{n}")))
                }
                _ => None,
            };
            match next {
                Some(next) => *v = next,
                None => bind(v, renames),
            }
        }),
        Value::Array(list) => list.iter_mut().for_each(|v| bind(v, renames)),
        _ => {}
    }
}

fn common(manifest: &Value) -> Option<String> {
    let manifest: Manifest = serde_json::from_value(manifest.clone()).ok()?;
    let maps: Vec<BTreeSet<String>> = manifest
        .texts()
        .into_iter()
        .filter_map(|(_, text)| match text {
            Text::Localized(map) => Some(map.keys().map(|l| tag(l)).collect()),
            Text::Plain(_) => None,
        })
        .collect();
    let shared = |language: &&String| maps.iter().all(|m| m.contains(*language));
    [String::from("en")].iter().chain(maps.first()?).find(shared).cloned()
}

pub fn upgrade(
    source: impl AsRef<Path>,
    content: Content,
    photosensitive: bool,
    origin: Origin,
) -> Result<(Manifest, Renames)> {
    let source = source.as_ref();
    let bytes = fs::read(source.join(MANIFEST))?;
    let first = Manifest::read(&bytes, false)?;
    check_path(&first.entry)?;
    let entry = (first.kind == Kind::Scene).then(|| source.join(&first.entry));
    let read = |path: &Path| -> Result<Value> { Ok(serde_json::from_slice(&fs::read(path)?)?) };
    let mut scene = entry.as_deref().map(read).transpose()?;
    let mut json: Value = serde_json::from_slice(&bytes)?;
    let renames = upgrade_json(&mut json, scene.as_mut())?;
    let mut manifest: Manifest = serde_json::from_value(json)?;
    (manifest.content, manifest.photosensitive, manifest.origin) =
        (Some(content), Some(photosensitive), Some(origin));
    let facts = facts_of(source, &manifest, &bytes, scene.as_ref().map(Value::to_string))?;
    manifest.inputs = facts.inputs.into_iter().collect();
    let features = facts.features.into_iter().collect::<Vec<_>>();
    manifest.requires = (!features.is_empty()).then_some(crate::Requires { engine: None, features });
    if let (Some(path), Some(scene)) = (entry.filter(|_| !renames.is_empty()), &scene) {
        fs::write(path, serde_json::to_string_pretty(scene)? + "\n")?;
    }
    fs::write(source.join(MANIFEST), manifest.to_json()? + "\n")?;
    let _ = fs::remove_file(source.join(SIGNATURE));
    Ok((manifest, renames))
}
