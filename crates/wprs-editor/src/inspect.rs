use crate::keys::value_at;
use crate::layers::{assets_of, walk};
use crate::meta::{ORDER, adjust, hidden};
use crate::schema::{MANIFEST, PROPERTY, at, choose, deref, root_of};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use wprs_scene::{Descriptor, builtin};

mod branches;
mod walk;

use walk::Walk;

pub type Own = BTreeMap<String, Arc<Descriptor>>;
static NONE: Own = BTreeMap::new();

pub(crate) const SOURCES: [&str; 6] = ["keys", "prop", "audio", "cursor", "clock", "script"];
const HIDDEN: [&str; 4] = ["type", "layers", "id", "schema"];
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Field {
    pub pointer: String,
    pub key: String,
    pub widget: String,
    pub value: Value,
    pub default: Value,
    pub set: bool,
    pub options: Vec<Value>,
    pub range: Value,
    pub unit: String,
    pub bindable: bool,
    pub source: String,
    pub depth: usize,
    pub sample: Value,
    pub shown: Value,
    pub required: bool,
    pub fixed: Vec<Value>,
    pub title: String,
    pub names: Vec<String>,
    pub help: String,
}

pub(crate) fn candidates(doc: &Value, reference: &str, pointer: &str) -> Vec<Value> {
    if let Some(found) = crate::settings::candidates(doc, reference, pointer) {
        return found;
    }
    let keys =
        |p: &str| doc.pointer(p).and_then(Value::as_object).map(|o| o.keys().map(|k| json!(k)).collect());
    let ids = |p: &str| {
        doc.pointer(p).and_then(Value::as_array).map(|l| l.iter().map(|t| t["id"].clone()).collect())
    };
    match reference {
        "layer" => walk(doc).into_iter().map(|(_, l)| l["id"].clone()).collect(),
        "system" => keys("/scene/particles").unwrap_or_default(),
        "timeline" => ids("/scene/timelines").unwrap_or_default(),
        "language" => crate::meta::languages(doc),
        "effect" => {
            let own =
                doc["scene"]["effects"].as_object().into_iter().flat_map(|o| o.keys().map(|k| json!(k)));
            builtin().iter().map(|d| json!(d.id)).chain(own).collect()
        }
        kind => assets_of(doc, kind).into_iter().map(|a| json!(a)).collect(),
    }
}

pub fn fields_at(doc: &Value, pointer: &str, time: f64) -> Vec<Field> {
    fields_with(doc, pointer, time, &NONE)
}

pub fn fields_with(doc: &Value, pointer: &str, time: f64, own: &Own) -> Vec<Field> {
    let mut out = fields_own(doc, pointer, own);
    for f in &mut out {
        f.shown = match f.source.as_str() {
            "keys" => value_at(&f.value, time),
            "" if f.set => f.value.clone(),
            "" => f.default.clone(),
            _ => Value::Null,
        };
    }
    out
}

pub fn fields(doc: &Value, pointer: &str) -> Vec<Field> {
    fields_own(doc, pointer, &NONE)
}

pub fn fields_own(doc: &Value, pointer: &str, own: &Own) -> Vec<Field> {
    let (Some((root, _)), Some(schema)) = (root_of(pointer), at(doc, pointer)) else { return Vec::new() };
    let value = doc.pointer(pointer).cloned().unwrap_or_default();
    let top = hidden(doc);
    let (hidden, order): (&[&str], &str) = match pointer {
        "/manifest" => (&top, ORDER),
        _ if std::ptr::eq(root, &*MANIFEST) => (&[], ORDER),
        _ if std::ptr::eq(root, &*PROPERTY) => (&HIDDEN, crate::settings::ORDER),
        _ => (&HIDDEN, ""),
    };
    let mut walk = Walk { doc, own, root, hidden, order, out: Vec::new() };
    let (mut resolved, _) = deref(root, &schema);
    if value.is_object() && !resolved["properties"].is_object() && resolved.get("oneOf").is_none() {
        resolved = deref(root, choose(root, resolved, &value)).0;
    }
    match resolved["properties"].is_object() || resolved.get("oneOf").is_some() {
        true => walk.object(resolved, &value, pointer, 0),
        false => walk.field(
            &schema,
            doc.pointer(pointer),
            pointer.into(),
            pointer.rsplit('/').next().unwrap_or(""),
            0,
        ),
    }
    crate::forms::extend(doc, pointer, &mut walk.out);
    walk.out.retain(|f| crate::meta::applies(doc, &f.pointer));
    walk.out.iter_mut().for_each(|f| adjust(doc, f));
    walk.out
}
