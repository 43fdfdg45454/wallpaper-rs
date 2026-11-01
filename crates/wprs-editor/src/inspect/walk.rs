use super::branches::{array, tagged};
use super::rigrefs::{RIG, options};
use super::{Field, Own, SOURCES, candidates};
use crate::schema::{branches, choose, deref, sample};
use crate::texts::rows;
use serde_json::Value;
use std::path::Path;
use wprs_scene::easings;

pub(super) struct Walk<'a> {
    pub(super) dir: Option<&'a Path>,
    pub(super) doc: &'a Value,
    pub(super) own: &'a Own,
    pub(super) root: &'a Value,
    pub(super) hidden: &'a [&'a str],
    pub(super) order: &'a str,
    pub(super) out: Vec<Field>,
}

impl Walk<'_> {
    pub(super) fn field(
        &mut self,
        meta: &Value,
        value: Option<&Value>,
        pointer: String,
        key: &str,
        depth: usize,
    ) {
        let (mut schema, mut name) = deref(self.root, meta);
        let current = value.cloned().unwrap_or(Value::Null);
        let mut f = Field {
            pointer: pointer.clone(),
            key: key.into(),
            value: current.clone(),
            set: value.is_some(),
            depth,
            ..Field::default()
        };
        f.default = meta.get("default").or(schema.get("default")).cloned().unwrap_or_default();
        f.range = meta.get("x-range").cloned().unwrap_or_default();
        f.unit = meta["x-unit"].as_str().unwrap_or_default().into();
        let list = branches(schema);
        let optional = list.len() == 2 && list.iter().any(|b| b["type"] == "null");
        if optional {
            (schema, name) = deref(self.root, list.iter().find(|b| b["type"] != "null").unwrap_or(schema));
        }
        let probe = if current.is_null() { &f.default } else { &current };
        let scalar = probe.is_array() || probe.is_number() || probe.is_boolean();
        let untyped = schema.get("type").is_none() && !name.starts_with("Value_for_") && scalar;
        if untyped && !branches(schema).is_empty() && !tagged(self.root, schema) {
            (schema, name) = deref(self.root, choose(self.root, schema, probe));
        }
        if name.starts_with("Value_for_") {
            f.bindable = true;
            f.source = SOURCES.iter().find(|s| current.get(**s).is_some()).map_or("", |s| *s).into();
            (schema, name) = deref(self.root, &branches(schema)[0]);
        }
        let types = match &schema["type"] {
            Value::Array(list) => list.iter().find(|t| *t != "null").cloned().unwrap_or_default(),
            t => t.clone(),
        };
        let reference = meta["x-ref"].as_str().or(schema["x-ref"].as_str());
        if let Some(r) = reference.filter(|r| RIG.contains(r))
            && let Some(found) = self.dir.and_then(|d| options(d, self.doc, r, &pointer, &current))
        {
            f.options = found;
            return self.rigged(f, r, schema, &current);
        }
        let reference = reference.filter(|r| !RIG.contains(r));
        let all: Vec<&Value> = branches(schema).iter().map(|b| deref(self.root, b).0).collect();
        let banded =
            all.iter().all(|b| b.get("enum").is_some() || b["type"] == "integer" || b["type"] == "array");
        let listed = all.iter().find_map(|b| b["enum"].as_array().cloned()).filter(|_| banded);
        let named: Vec<Value> = all.iter().filter_map(|b| b.get("const").cloned()).collect();
        let counted =
            all.iter().filter_map(|b| b["type"].as_str()).find(|t| ["integer", "number"].contains(t));
        let counted = counted.filter(|_| !named.is_empty() && named.len() + 1 == all.len());
        f.widget = match (name, types.as_str().unwrap_or_default()) {
            _ if meta["x-computed"] == true => "computed".into(),
            _ if meta["x-condition"] == true => "condition".into(),
            _ if meta["x-scalar"] == true => "scalar".into(),
            _ if meta["x-color"] == true => "color".into(),
            ("Color", _) => "color".into(),
            ("Vec2", _) => "vec2".into(),
            ("Ease", _) => {
                f.options = easings().into_iter().map(Value::from).collect();
                "enum".into()
            }
            ("Param", _) => "json".into(),
            ("Text", _) => {
                f.options = rows(self.doc, &pointer, &current);
                if meta["x-markup"] == true { "markup" } else { "texts" }.into()
            }
            _ if schema.get("const").is_some() => return,
            _ if let Some(reference) = reference => {
                f.options = candidates(self.doc, reference, &pointer);
                f.names = crate::settings::names(self.doc, reference, &f.options);
                format!("ref:{reference}")
            }
            _ if schema.get("enum").is_some() || listed.is_some() => {
                f.options = schema["enum"].as_array().cloned().or(listed).unwrap_or_default();
                "enum".into()
            }
            (_, "boolean") => "bool".into(),
            (_, "integer") => "integer".into(),
            (_, "number") => "number".into(),
            _ if counted.is_some() => {
                f.options = named;
                counted.unwrap_or_default().into()
            }
            (_, "string") => "text".into(),
            (_, "array") => array(self.root, meta, schema),
            (_, "object") if optional && value.is_none() => {
                return self.out.push(Field {
                    widget: "group".into(),
                    sample: sample(self.root, schema),
                    ..f
                });
            }
            (_, "object") if schema["properties"].is_object() && depth < 3 => {
                self.out.push(Field { widget: "group".into(), ..f });
                return self.object(schema, &current, &pointer, depth + 1);
            }
            _ if tagged(self.root, schema) && depth < 3 => return self.variant(f, schema, &current),
            _ => "json".into(),
        };
        let items = schema.get("items").unwrap_or(&Value::Null);
        match f.widget.as_str() {
            "list" => f.sample = sample(self.root, items),
            "flags" => f.options = deref(self.root, items).0["enum"].as_array().cloned().unwrap_or_default(),
            _ => {}
        }
        self.out.push(f);
    }
}
