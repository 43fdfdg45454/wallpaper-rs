use crate::ops::{Op, set};
use anyhow::{Result, ensure};
use serde_json::{Map, Value, json};
use std::path::Path;
use wprs_core::message::said;

const PLAYBACK: [&str; 7] = ["max_fps", "volume", "muted", "speed", "start", "end", "loop"];
const VIDEO_ONLY: [&str; 4] = ["speed", "start", "end", "loop"];
const WEB: [&str; 3] = ["preload", "background", "storage_kib"];
pub const INPUTS: [&str; 5] = ["audio", "pointer", "clicks", "media", "network"];

fn strings(value: &Value) -> Vec<String> {
    value.as_array().into_iter().flatten().filter_map(|v| v.as_str().map(String::from)).collect()
}

fn merged(old: Vec<String>, given: &[String], add: bool, remove: bool) -> Vec<String> {
    let mut next = match (add, remove) {
        (true, _) => old.into_iter().chain(given.iter().cloned()).collect(),
        (_, true) => old.into_iter().filter(|o| !given.contains(o)).collect(),
        _ => given.to_vec(),
    };
    let mut seen = Vec::new();
    next.retain(|item| {
        let fresh = !seen.contains(item);
        seen.push(item.clone());
        fresh
    });
    next
}

fn allowed(kind: &str, section: &str) -> Vec<&'static str> {
    match (section, kind) {
        ("web", "web") => WEB.to_vec(),
        ("playback", "video") => PLAYBACK.to_vec(),
        ("playback", "web" | "scene") => {
            PLAYBACK.iter().copied().filter(|k| !VIDEO_ONLY.contains(k)).collect()
        }
        _ => Vec::new(),
    }
}

pub(super) fn section(
    doc: &Value,
    name: &str,
    values: &Map<String, Value>,
    clear: &[String],
) -> Result<Vec<Op>> {
    let kind = crate::pages::kind(doc);
    let known = allowed(kind, name);
    for key in values.keys().chain(clear) {
        ensure!(
            known.contains(&key.as_str()),
            said(
                "editor.no_section_setting",
                "%1 is not a %2 setting of a %3 package; it takes %4",
                &[key, &name, &kind, &(if known.is_empty() { "none".into() } else { known.join(", ") })]
            )
        );
    }
    let mut next = doc["manifest"][name].as_object().cloned().unwrap_or_default();
    clear.iter().for_each(|k| drop(next.remove(k)));
    values.iter().for_each(|(k, v)| drop(next.insert(k.clone(), v.clone())));
    let at = format!("/manifest/{name}");
    Ok(super::effects::put(doc, &at, (!next.is_empty()).then_some(Value::Object(next))))
}

fn origin(text: &str) -> String {
    text.trim().trim_end_matches('/').to_ascii_lowercase()
}

pub(super) fn network(doc: &Value, origins: &[String], add: bool, remove: bool) -> Result<Vec<Op>> {
    ensure!(
        crate::pages::kind(doc) == "web",
        said("editor.network_web_only", "only a web page may reach the network", &[])
    );
    let given: Vec<String> = origins.iter().map(|o| origin(o)).filter(|o| !o.is_empty()).collect();
    let old = strings(&doc["manifest"]["permissions"]["network"]["allow"]);
    let next = merged(old, &given, add, remove);
    let mut permissions = doc["manifest"]["permissions"].as_object().cloned().unwrap_or_default();
    match next.is_empty() {
        true => drop(permissions.remove("network")),
        false => drop(permissions.insert("network".into(), json!({ "allow": next }))),
    }
    let kept = (!permissions.is_empty()).then_some(Value::Object(permissions));
    let mut ops = super::effects::put(doc, "/manifest/permissions", kept);
    let mut inputs = strings(&doc["manifest"]["inputs"]);
    inputs.retain(|i| i != "network");
    if !next.is_empty() {
        inputs.push("network".into());
    }
    if strings(&doc["manifest"]["inputs"]) != inputs {
        ops.extend(set(doc, "/manifest/inputs", json!(inputs)));
    }
    Ok(ops)
}

pub(super) fn inputs(doc: &Value, inputs: &[String], add: bool, remove: bool) -> Result<Vec<Op>> {
    for input in inputs {
        ensure!(
            INPUTS.contains(&input.as_str()),
            said("editor.not_input", "%1 is not an input; use one of %2", &[input, &INPUTS.join(", ")])
        );
    }
    let next = merged(strings(&doc["manifest"]["inputs"]), inputs, add, remove);
    let listed = !strings(&doc["manifest"]["permissions"]["network"]["allow"]).is_empty();
    ensure!(
        next.contains(&"network".to_owned()) == listed || crate::pages::kind(doc) != "web",
        said(
            "editor.network_follows_origins",
            "a page declares network exactly when it lists the origins it reaches",
            &[]
        )
    );
    Ok(set(doc, "/manifest/inputs", json!(next)))
}

pub(super) fn add_file(dir: &Path, doc: &Value, file: &Path, to: Option<&str>) -> Result<Vec<Op>> {
    ensure!(
        !crate::pages::scened(doc),
        said(
            "editor.page_files_only",
            "files are added this way only to web, picture and video packages",
            &[]
        )
    );
    crate::pages::copy_in(dir, file, to)?;
    Ok(Vec::new())
}
