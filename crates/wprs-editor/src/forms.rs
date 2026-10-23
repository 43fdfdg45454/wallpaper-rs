use crate::inspect::{Field, fields};
use crate::listing::{named, ordered, values as options};
use crate::ops::escape;
use crate::settings::{candidates, names, properties};
use serde_json::{Value, json};
use wprs_scene::Control;

pub fn extend(doc: &Value, pointer: &str, out: &mut Vec<Field>) {
    let preset = pointer.strip_prefix("/manifest/presets/").is_some_and(|n| n.parse::<usize>().is_ok());
    if preset {
        out.retain(|f| f.key != "values");
        out.extend(values(doc, pointer));
    }
    let value = doc.pointer(pointer).unwrap_or(&Value::Null);
    if let (Some(id), None) = (value.get("prop").and_then(Value::as_str), value.get("keys")) {
        variants(doc, pointer, id, out);
    }
}

fn group(pointer: String, key: &str) -> Field {
    Field { pointer, key: key.into(), widget: "group".into(), set: true, required: true, ..Field::default() }
}

fn values(doc: &Value, pointer: &str) -> Vec<Field> {
    let base = format!("{pointer}/values");
    let set = doc.pointer(&base);
    let props = properties(doc);
    let shown =
        ordered(doc).into_iter().filter(|id| props.get(id).is_some_and(|p| p.holds_value() && !p.personal()));
    let field = |id: String| {
        let (p, current) = (&props[&id], set.and_then(|v| v.get(&id)));
        let mut f = Field {
            pointer: format!("{base}/{}", escape(&id)),
            title: named(doc, &id),
            value: current.cloned().unwrap_or_default(),
            default: p.default_value(),
            set: current.is_some(),
            depth: 1,
            key: id,
            ..Field::default()
        };
        f.widget = match &p.control {
            Control::Slider(s) => {
                (f.range, f.unit) = (json!([s.min, s.max]), s.unit.clone().unwrap_or_default());
                if s.integer { "integer" } else { "number" }
            }
            Control::Combo { .. } => {
                (f.options, f.names) =
                    options(doc, &doc["manifest"]["properties"][&f.key]).into_iter().unzip();
                "ref:option"
            }
            Control::Color { alpha: true, .. } => "color",
            Control::Color { .. } => "rgb",
            Control::Bool { .. } => "bool",
            _ => "text",
        }
        .into();
        f
    };
    [group(base.clone(), "values")].into_iter().chain(shown.map(field)).collect()
}

fn variants(doc: &Value, pointer: &str, id: &str, out: &mut Vec<Field>) {
    let prop = &doc["manifest"]["properties"][id];
    let text = |v: &Value| v.as_str().map_or_else(|| v.to_string(), String::from);
    let keys: Vec<(String, String)> = match prop["type"].as_str() {
        Some("combo") => options(doc, prop).into_iter().map(|(v, name)| (text(&v), name)).collect(),
        Some("bool") => vec![("false".into(), String::new()), ("true".into(), String::new())],
        _ => return,
    };
    let parent = pointer.rsplit_once('/').map_or("", |(p, _)| p);
    let parent = parent.strip_suffix("/params").unwrap_or(parent);
    let Some(base) = fields(doc, parent).into_iter().find(|f| f.pointer == pointer) else { return };
    let map = doc.pointer(pointer).and_then(|v| v.get("map"));
    let made = || {
        let options = candidates(doc, "property", pointer).unwrap_or_default();
        let names = names(doc, "property", &options);
        let (pointer, value) = (format!("{pointer}/prop"), json!(id));
        Field {
            pointer,
            key: "prop".into(),
            widget: "ref:property".into(),
            value,
            set: true,
            options,
            names,
            ..Field::default()
        }
    };
    let prop = out.iter().position(|f| f.key == "prop").map(|i| out.remove(i)).unwrap_or_else(made);
    out.retain(|f| f.key != "map" && f.pointer != pointer);
    let row = |(key, title): (String, String)| {
        let value = map.and_then(|m| m.get(&key));
        Field {
            pointer: format!("{pointer}/map/{}", escape(&key)),
            key,
            title,
            widget: base.widget.clone(),
            options: base.options.clone(),
            range: base.range.clone(),
            unit: base.unit.clone(),
            value: value.cloned().unwrap_or_default(),
            set: value.is_some(),
            depth: 1,
            ..Field::default()
        }
    };
    let header = group(format!("{pointer}/map"), "map");
    let rows: Vec<Field> = [prop, header].into_iter().chain(keys.into_iter().map(row)).collect();
    out.splice(0..0, rows);
}
