use crate::ops::Op;
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use wprs_scene::{MAX_SCRIPT_BYTES, MAX_SCRIPTS2, ScriptApi, ScriptEvent, imported};

const CALLS: [(&str, ScriptApi); 4] = [
    (".spawn(", ScriptApi::Layers),
    (".order(", ScriptApi::Layers),
    (".effect(", ScriptApi::Effects),
    ("console.", ScriptApi::Console),
];

fn exported(source: &str) -> Vec<String> {
    let mut found = Vec::new();
    for line in source.lines().map(str::trim_start).filter_map(|l| l.strip_prefix("export ")) {
        let line = line.trim_start_matches("async ");
        let named = ["function ", "const ", "let ", "var "].iter().find_map(|k| line.strip_prefix(k));
        let names: Vec<&str> = match named {
            Some(rest) => rest.split(|c: char| !(c.is_alphanumeric() || c == '_')).take(1).collect(),
            None => line.trim_start_matches('{').split('}').next().unwrap_or_default().split(',').collect(),
        };
        let last = |n: &str| n.split_whitespace().last().unwrap_or_default().to_owned();
        found.extend(names.into_iter().map(last).filter(|n| !n.is_empty()));
    }
    found
}

pub fn declaration(source: &str) -> Value {
    let names = exported(source);
    let mut events: Vec<ScriptEvent> =
        names.iter().filter_map(|n| serde_json::from_value(json!(n)).ok()).collect();
    for touch in ["click", "enter", "leave", "move", "down", "up"] {
        if [format!(".on(\"{touch}\""), format!(".on('{touch}'")].iter().any(|t| source.contains(t.as_str()))
        {
            events.extend(serde_json::from_value::<ScriptEvent>(json!(touch)));
        }
    }
    events.sort();
    events.dedup();
    let mut uses = imported(source);
    uses.extend(CALLS.iter().filter(|(call, _)| source.contains(call)).map(|(_, api)| *api));
    uses.sort();
    uses.dedup();
    let mut entry = json!({});
    if events.iter().any(|e| *e != ScriptEvent::Update) {
        entry["events"] = json!(events);
    }
    if !uses.is_empty() {
        entry["uses"] = json!(uses);
    }
    entry
}

pub fn import(dir: &Path, doc: &Value, file: &Path) -> Result<Vec<Op>> {
    let name = file.file_name().and_then(|n| n.to_str()).context("the script has no file name")?;
    ensure!(name.ends_with(".js"), "{name} is not a .js file");
    let size = fs::metadata(file).with_context(|| format!("cannot read {}", file.display()))?.len();
    let list = doc.pointer("/scene/scripts").and_then(Value::as_array).cloned().unwrap_or_default();
    let path = format!("scripts/{name}");
    let others = list.iter().filter_map(|s| s["path"].as_str()).filter(|p| *p != path);
    let total: u64 = others.clone().filter_map(|p| fs::metadata(dir.join(p)).ok()).map(|m| m.len()).sum();
    ensure!(size + total <= MAX_SCRIPT_BYTES as u64, "the scripts of a scene take at most 1 MiB together");
    ensure!(others.count() < MAX_SCRIPTS2, "a scene has at most {MAX_SCRIPTS2} scripts");
    let source =
        fs::read_to_string(file).with_context(|| format!("{} is not a text file", file.display()))?;
    fs::create_dir_all(dir.join("scripts"))?;
    fs::write(dir.join(&path), &source).with_context(|| format!("cannot write {path}"))?;
    let mut entry = declaration(&source);
    entry["path"] = json!(path);
    Ok(match list.iter().position(|s| s["path"] == path) {
        Some(i) => vec![Op::Replace { path: format!("/scene/scripts/{i}"), value: entry }],
        None if list.is_empty() && doc.pointer("/scene/scripts").is_none() => {
            vec![Op::Add { path: "/scene/scripts".into(), value: json!([entry]) }]
        }
        None => vec![Op::Add { path: "/scene/scripts/-".into(), value: entry }],
    })
}
