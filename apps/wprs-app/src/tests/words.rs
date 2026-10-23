use super::qml_files;
use serde_json::{Value, json};
use std::collections::{BTreeSet, HashMap};
use wprs_editor::actions::plan;
use wprs_editor::inspect::fields_at;
use wprs_editor::layers::{KINDS, asset_kind, walk};
use wprs_editor::ops::{apply, set};
use wprs_editor::schema::{PROPERTY, branches, sample};
use wprs_editor::starters::{STARTERS, starter};
use wprs_editor::{Action, Field};

type Tables = HashMap<String, HashMap<String, String>>;

fn tables() -> Tables {
    let mut out = Tables::new();
    for (file, text) in qml_files().into_iter().filter(|(n, _)| n.ends_with("Words.qml") || n == "Labels.qml")
    {
        for block in text.split("readonly property var ").skip(1) {
            let Some((name, body)) = block.split_once(": ({").filter(|(n, _)| !n.contains(' ')) else {
                continue;
            };
            let rows = body.split("})").next().unwrap().lines().filter_map(|line| {
                let (key, text) = line.trim().split_once("\": App.tr(\"")?;
                Some((
                    key.trim_start_matches('"').into(),
                    text.trim_end_matches(',').strip_suffix("\")")?.into(),
                ))
            });
            out.entry(format!("{}.{name}", &file[..1])).or_default().extend(rows);
        }
    }
    out
}

struct Words(Tables);

impl Words {
    fn get(&self, table: &str, key: &str) -> Option<String> {
        self.0.get(table)?.get(key).cloned()
    }

    fn label(&self, key: &str, parent: &str) -> Option<String> {
        let keys = [format!("{parent}.{key}"), key.to_owned()];
        keys.iter().find_map(|k| self.get("S.words", k).or_else(|| self.get("M.words", k)))
    }

    fn value(&self, text: &str) -> Option<String> {
        let tables = ["S.values", "M.values", "S.kinds", "S.effects"];
        let found = tables.iter().find_map(|t| self.get(t, text));
        found.or_else(|| self.label(text, "")).or_else(|| self.easing(text))
    }

    fn easing(&self, name: &str) -> Option<String> {
        let ways = [("in_out_", "Ease in and out (%1)"), ("in_", "Ease in (%1)"), ("out_", "Ease out (%1)")];
        let (curve, text) = ways.iter().find_map(|(w, t)| Some((name.strip_prefix(w)?, *t)))?;
        self.get("S.curves", curve).map(|_| text.to_owned())
    }
}

fn menus(words: &Words) -> Vec<(String, Option<String>)> {
    let kind = |k: &str| words.get("S.kinds", k).or_else(|| words.label(k, ""));
    let mut out: Vec<_> = KINDS.iter().chain(&STARTERS).map(|&k| (k.to_owned(), kind(k))).collect();
    out.extend(
        ["a.png", "a.webm", "a.ogg", "a.ttf", "a.cube", "a.txt"].map(|p| (p.into(), kind(asset_kind(p)))),
    );
    out.extend(wprs_scene::builtin().iter().map(|d| (d.id.clone(), words.get("S.effects", &d.id))));
    for e in wprs_scene::easings() {
        let curve = e.rsplit_once('_').map(|(_, c)| c).filter(|_| words.easing(&e).is_some());
        out.extend(curve.map(|c| (e.clone(), words.get("S.curves", c))));
        out.push((e.clone(), words.easing(&e).or_else(|| words.value(&e))));
    }
    let types = branches(&PROPERTY).iter().filter_map(|b| b.pointer("/properties/type/const")?.as_str());
    out.extend(types.map(|t| (t.into(), words.label(t, ""))));
    out.extend(wprs_format::features::REGISTRY.map(|(f, _)| (f.into(), words.get("L.features", f))));
    out
}

fn document(name: &str) -> Value {
    let mut doc = starter(name, "org.example.words", "Words", "en").unwrap();
    let assets =
        [("pic", "a.png"), ("clip", "a.webm"), ("tone", "a.ogg"), ("face", "a.ttf"), ("grade", "a.cube")];
    for (id, path) in assets {
        let ops = set(&doc, &format!("/scene/assets/{id}"), json!({"path": format!("assets/{path}")}));
        apply(&mut doc, &ops).unwrap();
    }
    for kind in KINDS {
        let source =
            [("image", "pic"), ("video", "clip"), ("sound", "tone")].into_iter().find(|(k, _)| *k == kind);
        let source = source.map(|(_, s)| s.to_owned());
        let layer =
            Action::AddLayer { kind: kind.into(), id: None, name: None, source, into: None, at: None };
        let ops = plan(&doc, &layer);
        apply(&mut doc, &ops.unwrap()).unwrap();
    }
    for (i, branch) in branches(&PROPERTY).iter().enumerate() {
        let ops = set(&doc, &format!("/manifest/properties/p{i}"), sample(&PROPERTY, branch));
        apply(&mut doc, &ops).unwrap();
    }
    doc
}

fn explore(doc: &Value, pointer: &str, seen: &mut BTreeSet<String>, shown: &mut Vec<Field>) {
    let shape: Vec<&str> =
        pointer.split('/').map(|t| if t.parse::<u32>().is_ok() { "#" } else { t }).collect();
    let at = doc.pointer(pointer).cloned().unwrap_or_default();
    let keys: Vec<&String> = at.as_object().map(|o| o.keys().collect()).unwrap_or_default();
    if shape.len() > 12 || !seen.insert(format!("{shape:?} {keys:?} {}", at["type"])) {
        return;
    }
    for field in fields_at(doc, pointer, 0.0) {
        let samples: Vec<Value> = match (field.widget.as_str(), field.set) {
            ("variant", _) => {
                field.sample.as_object().map(|o| o.values().cloned().collect()).unwrap_or_default()
            }
            ("group", false) => vec![field.sample.clone()],
            ("list", false) => vec![json!([field.sample])],
            ("group" | "list", true) => vec![field.value.clone()],
            _ => Vec::new(),
        };
        let bound = (field.bindable && seen.insert("bound".into())).then(|| json!({"audio": "bass"}));
        for value in samples.into_iter().chain(bound).filter(|v| !v.is_null()) {
            let mut copy = doc.clone();
            if apply(&mut copy, &set(doc, &field.pointer, value.clone())).is_ok() {
                explore(&copy, &field.pointer, seen, shown);
                let items = value.as_array().map_or(0, Vec::len);
                (0..items).for_each(|i| explore(&copy, &format!("{}/{i}", field.pointer), seen, shown));
            }
        }
        shown.push(field);
    }
}

#[test]
fn every_value_and_label_the_scene_editor_shows_is_translated() {
    let (words, spanish) = (Words(tables()), crate::tr::catalog(&["es".to_owned()]));
    let (mut seen, mut shown) = (BTreeSet::new(), Vec::new());
    for doc in STARTERS.map(document) {
        let properties = doc["manifest"]["properties"].as_object().unwrap().keys();
        let systems = doc["scene"]["particles"].as_object().into_iter().flat_map(|p| p.keys());
        let mut roots: Vec<String> = walk(&doc).into_iter().map(|(pointer, _)| pointer).collect();
        roots.extend(["/scene".into(), "/manifest".into()]);
        roots.extend(properties.map(|p| format!("/manifest/properties/{p}")));
        roots.extend(systems.map(|s| format!("/scene/particles/{s}")));
        roots.iter().for_each(|root| explore(&doc, root, &mut seen, &mut shown));
    }
    let mut wanted = menus(&words);
    for f in &shown {
        let parent = f.pointer.rsplit('/').nth(1).unwrap_or_default();
        if f.key.parse::<u32>().is_err() {
            wanted.push((f.pointer.clone(), words.label(&f.key, parent)));
        }
        let named =
            ["enum", "variant", "flags", "integer", "number", "ref:effect"].contains(&f.widget.as_str());
        for option in f.options.iter().filter_map(Value::as_str) {
            let language = option.split('-').next().unwrap_or_default();
            let text = match f.widget.as_str() {
                "ref:language" => words.get("M.languages", language),
                _ if !named => continue,
                _ if f.key == "inputs" => words.get("M.inputs", option).or_else(|| words.value(option)),
                _ => words.value(option),
            };
            wanted.push((format!("{} {option}", f.pointer), text));
        }
        let free = f.widget.starts_with("ref:") && f.widget != "ref:effect";
        match (f.shown.as_str(), f.widget.as_str()) {
            (Some(raw), "json") => wanted.push((format!("{} shows the raw value {raw:?}", f.pointer), None)),
            (_, "text" | "texts" | "markup" | "color" | "rgb" | "lines") => {}
            (Some(text), _) if !free && !text.is_empty() => {
                wanted.push((format!("{} {text}", f.pointer), words.value(text)))
            }
            _ => {}
        }
    }
    let values: BTreeSet<&str> =
        shown.iter().flat_map(|f| f.options.iter().filter_map(Value::as_str)).collect();
    assert!(
        ["ellipse", "soft_light", "bloom", "luminance", "edge", "inside", "average", "pingpong"]
            .iter()
            .all(|v| values.contains(v)),
        "{values:?}"
    );
    let missing: BTreeSet<String> = wanted
        .into_iter()
        .filter(|(_, id)| id.as_ref().and_then(|id| spanish.get(id)).is_none_or(String::is_empty))
        .map(|(what, id)| format!("{what} ({})", id.unwrap_or_default()))
        .collect();
    assert!(missing.is_empty(), "the scene editor shows these without a Spanish translation: {missing:#?}");
}
