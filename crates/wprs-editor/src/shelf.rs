use anyhow::{Result, bail, ensure};
use serde_json::{Map, Value, json};
use std::collections::BTreeSet;
use wprs_core::message::said;
use wprs_format::PART_KINDS;

pub const NAMED: [&str; 5] = ["assets", "particles", "effects", "templates", "pools"];
pub const LISTED: [&str; 3] = ["layers", "scripts", "timelines"];

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pick {
    pub kind: String,
    pub id: String,
}

impl Pick {
    pub fn parse(text: &str) -> Result<Pick> {
        let (kind, id) = text.split_once(':').unwrap_or(("", text));
        ensure!(
            PART_KINDS.contains(&kind) && !id.is_empty(),
            said(
                "editor.part_named",
                "name a part as layer:ID, particles:ID, script:PATH or effect:ID, not %1",
                &[&text]
            )
        );
        Ok(Pick { kind: kind.into(), id: id.into() })
    }

    pub fn key(&self) -> String {
        format!("{}:{}", self.kind, self.id)
    }
}

fn layer<'a>(list: &'a Value, id: &str) -> Option<&'a Value> {
    let items = list.as_array()?;
    items.iter().find(|l| l["id"] == id).or_else(|| items.iter().find_map(|l| layer(&l["layers"], id)))
}

pub fn strings(value: &Value, out: &mut BTreeSet<String>) {
    match value {
        Value::String(text) => drop(out.insert(text.clone())),
        Value::Array(items) => items.iter().for_each(|v| strings(v, out)),
        Value::Object(map) => map.iter().filter(|(k, _)| *k != "text").for_each(|(_, v)| strings(v, out)),
        _ => {}
    }
}

fn picked(scene: &Value, pick: &Pick) -> Option<Value> {
    match pick.kind.as_str() {
        "layer" => layer(&scene["layers"], &pick.id).cloned(),
        "script" => scene["scripts"].as_array()?.iter().find(|s| s["path"] == pick.id.as_str()).cloned(),
        "particles" => scene["particles"].get(&pick.id).cloned(),
        _ => scene["effects"].get(&pick.id).cloned(),
    }
}

pub fn fragment(scene: &Value, picks: &[Pick]) -> Result<Value> {
    ensure!(!picks.is_empty(), said("editor.choose_parts", "choose at least one part", &[]));
    let mut out = json!({});
    for pick in picks {
        let Some(value) = picked(scene, pick) else {
            bail!(said("editor.no_such_part", "the scene has no %1", &[&pick.key()]))
        };
        match pick.kind.as_str() {
            "layer" => push(&mut out, "layers", value),
            "script" => push(&mut out, "scripts", value),
            "particles" => drop(entry(&mut out, "particles").insert(pick.id.clone(), value)),
            _ => drop(entry(&mut out, "effects").insert(pick.id.clone(), value)),
        }
    }
    loop {
        let mut names = BTreeSet::new();
        strings(&out, &mut names);
        let before = out.to_string().len();
        for section in NAMED {
            for (key, value) in scene[section].as_object().into_iter().flatten() {
                if names.contains(key) && out[section].get(key).is_none() {
                    entry(&mut out, section).insert(key.clone(), value.clone());
                }
            }
        }
        for timeline in scene["timelines"].as_array().into_iter().flatten() {
            let id = timeline["id"].as_str().unwrap_or_default();
            let known = out["timelines"].as_array().into_iter().flatten().any(|t| t["id"] == id);
            if names.contains(id) && !known {
                push(&mut out, "timelines", timeline.clone());
            }
        }
        if out.to_string().len() == before {
            return Ok(out);
        }
    }
}

fn push(out: &mut Value, section: &str, value: Value) {
    let list = out.as_object_mut().map(|o| o.entry(section).or_insert_with(|| json!([])));
    list.and_then(Value::as_array_mut).into_iter().for_each(|l| l.push(value.clone()));
}

fn entry<'a>(out: &'a mut Value, section: &str) -> &'a mut Map<String, Value> {
    let slot = out.as_object_mut().map(|o| o.entry(section).or_insert_with(|| json!({})));
    slot.and_then(Value::as_object_mut).expect("a fragment is an object of sections")
}

pub fn files(fragment: &Value) -> BTreeSet<String> {
    let assets = fragment["assets"].as_object().into_iter().flatten().map(|(_, a)| &a["path"]);
    let scripts = fragment["scripts"].as_array().into_iter().flatten().map(|s| &s["path"]);
    let effects = fragment["effects"].as_object().into_iter().flatten();
    let effects = effects.flat_map(|(_, e)| [&e["descriptor"], &e["wgsl"]]);
    assets.chain(scripts).chain(effects).filter_map(Value::as_str).map(String::from).collect()
}
