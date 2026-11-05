use crate::ops::Op;
use anyhow::{Result, ensure};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use wprs_core::message::said;
use wprs_scene::{MAX_SCRIPT_BYTES, MAX_SCRIPTS2, ScriptApi, ScriptEvent, imported};

const CALLS: [(&str, ScriptApi); 4] = [
    (".spawn(", ScriptApi::Layers),
    (".order(", ScriptApi::Layers),
    (".effect(", ScriptApi::Effects),
    ("console.", ScriptApi::Console),
];

const STAGE: [&str; 7] = ["camera", "camera3d", "parallax", "bloom", "background", "ambient", "sky"];

fn staged(source: &str) -> bool {
    let word = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    let left = |before: &str| match before.chars().next_back() {
        Some('.') => before.ends_with("wprs."),
        Some(c) => !word(c),
        None => true,
    };
    STAGE.iter().any(|key| {
        let call = format!("scene.{key}");
        let mut found = source.match_indices(&call);
        found.any(|(i, _)| left(&source[..i]) && !source[i + call.len()..].starts_with(word))
    })
}

fn unpacked(source: &str) -> bool {
    let word = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    source.match_indices('}').any(|(i, _)| {
        let rest = source[i + 1..].trim_start();
        let Some(rest) = rest.strip_prefix('=').map(str::trim_start) else { return false };
        let rest = rest.strip_prefix("wprs.").unwrap_or(rest);
        let named = rest.strip_prefix("scene").is_some_and(|after| !after.starts_with(word));
        let open = source[..i].rfind('{').unwrap_or(i);
        named && source[open + 1..i].split(|c: char| !word(c)).any(|w| STAGE.contains(&w))
    })
}

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
    uses.extend((staged(source) || unpacked(source)).then_some(ScriptApi::Camera));
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

pub fn import(dir: &Path, doc: &Value, file: &Path, template: Option<&str>) -> Result<Vec<Op>> {
    let free = template.filter(|_| file.as_os_str().is_empty()).map(|t| crate::molds::free_name(dir, t));
    let file = free.as_deref().map_or(file, Path::new);
    let name = file
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| said("editor.script_name", "the script has no file name", &[]))?;
    ensure!(name.ends_with(".js"), said("editor.not_js_file", "%1 is not a .js file", &[&name]));
    let made = template.map(|t| crate::molds::script_template(t).ok_or(t)).transpose();
    let made = made.map_err(|t| said("editor.no_template", "there is no script template %1", &[&t]))?;
    ensure!(
        made.is_none() || !dir.join("scripts").join(name).exists(),
        said("editor.script_taken", "scripts/%1 already exists", &[&name])
    );
    let size = match made {
        Some(source) => source.len() as u64,
        None => fs::metadata(file)
            .map_err(|e| said("editor.cannot_read", "cannot read %1: %2", &[&file.display(), &e]))?
            .len(),
    };
    let list = doc.pointer("/scene/scripts").and_then(Value::as_array).cloned().unwrap_or_default();
    let path = format!("scripts/{name}");
    let others = list.iter().filter_map(|s| s["path"].as_str()).filter(|p| *p != path);
    let total: u64 = others.clone().filter_map(|p| fs::metadata(dir.join(p)).ok()).map(|m| m.len()).sum();
    ensure!(
        size + total <= MAX_SCRIPT_BYTES as u64,
        said("editor.scripts_scene_take", "the scripts of a scene take at most 1 MiB together", &[])
    );
    ensure!(
        others.count() < MAX_SCRIPTS2,
        said("editor.scene_most_scripts", "a scene has at most %1 scripts", &[&MAX_SCRIPTS2])
    );
    let source = match made {
        Some(source) => source.to_owned(),
        None => fs::read_to_string(file)
            .map_err(|e| said("editor.not_text", "%1 is not a text file: %2", &[&file.display(), &e]))?,
    };
    fs::create_dir_all(dir.join("scripts"))?;
    fs::write(dir.join(&path), &source)
        .map_err(|e| said("editor.cannot_write", "cannot write %1: %2", &[&path, &e]))?;
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
