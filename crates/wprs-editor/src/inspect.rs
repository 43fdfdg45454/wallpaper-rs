use crate::keys::value_at;
use crate::layers::{assets_of, walk};
use crate::meta::{ORDER, adjust, hidden};
use crate::ops::escape;
use crate::schema::{MANIFEST, PROPERTY, at, branches, choose, deref, fits, properties, root_of, sample};
use crate::texts::rows;
use serde::Serialize;
use serde_json::{Value, json};
use wprs_scene::{ParamKind, descriptor, easings};

const SOURCES: [&str; 6] = ["keys", "prop", "audio", "cursor", "clock", "script"];
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
}

fn candidates(doc: &Value, reference: &str, pointer: &str) -> Vec<Value> {
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
        kind => assets_of(doc, kind).into_iter().map(|a| json!(a)).collect(),
    }
}

struct Walk<'a> {
    doc: &'a Value,
    root: &'a Value,
    hidden: &'a [&'a str],
    order: &'a str,
    out: Vec<Field>,
}

impl Walk<'_> {
    fn field(&mut self, meta: &Value, value: Option<&Value>, pointer: String, key: &str, depth: usize) {
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
        if name.starts_with("Value_for_") {
            f.bindable = true;
            f.source = SOURCES.iter().find(|s| current.get(**s).is_some()).map_or("", |s| *s).into();
            (schema, name) = deref(self.root, &branches(schema)[0]);
        }
        let types = match &schema["type"] {
            Value::Array(list) => list.iter().find(|t| *t != "null").cloned().unwrap_or_default(),
            t => t.clone(),
        };
        let reference = meta["x-ref"].as_str();
        let all = branches(schema).iter().map(|b| deref(self.root, b).0);
        let banded = all.clone().all(|b| b.get("enum").is_some() || b["type"] == "integer");
        let listed = all.filter_map(|b| b["enum"].as_array().cloned()).next().filter(|_| banded);
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

    fn variant(&mut self, mut f: Field, schema: &Value, current: &Value) {
        let tag = |b: &Value| tag(self.root, b);
        f.options = branches(schema).iter().filter_map(tag).collect();
        f.sample = Value::Object(
            branches(schema)
                .iter()
                .filter_map(|b| Some((tag(b)?.as_str()?.to_owned(), sample(self.root, b))))
                .collect(),
        );
        f.widget = "variant".into();
        let (pointer, depth) = (f.pointer.clone(), f.depth);
        self.out.push(f);
        let chosen = choose(self.root, schema, current);
        if let Some((key, inner)) = current.as_object().and_then(|o| o.iter().next()) {
            let meta = deref(self.root, chosen).0["properties"].get(key).cloned().unwrap_or_default();
            self.field(&meta, Some(inner), format!("{pointer}/{}", escape(key)), key, depth + 1);
        }
    }

    fn object(&mut self, schema: &Value, value: &Value, pointer: &str, depth: usize) {
        let (base, _) = deref(self.root, schema);
        let variants = base.get("oneOf").and_then(Value::as_array).into_iter().flatten();
        let variant = variants.filter(|v| fits(self.root, v, value)).map(|v| deref(self.root, v).0).next();
        let required: Vec<&Value> = [Some(base), variant]
            .into_iter()
            .flatten()
            .flat_map(|s| s["required"].as_array())
            .flatten()
            .collect();
        let mut all: Vec<(String, Value)> = properties(self.root, schema, value).into_iter().collect();
        let rank = |key: &str| self.order.split_whitespace().position(|o| o == key).unwrap_or(usize::MAX);
        all.sort_by_key(|(key, _)| rank(key));
        for (key, meta) in all {
            let first = self.out.len();
            if self.hidden.contains(&key.as_str()) && depth == 0 || key == "ext" {
                continue;
            }
            let path = format!("{pointer}/{}", escape(&key));
            match (key.as_str(), value["effect"].as_str().and_then(descriptor)) {
                ("params", Some(d)) => d.params.iter().for_each(|p| self.param(p, value, &path, depth)),
                _ => self.field(&meta, value.get(&key), path, &key, depth),
            }
            if let Some(f) = self.out.get_mut(first) {
                f.required = required.contains(&&json!(key)) || meta["x-required"] == true;
            }
        }
    }

    fn param(&mut self, p: &wprs_scene::ParamDef, value: &Value, base: &str, depth: usize) {
        let current = value["params"].get(&p.id);
        let (widget, reference) = match p.kind {
            ParamKind::Float => ("number", None),
            ParamKind::Vec2 => ("vec2", None),
            ParamKind::Color => ("color", None),
            ParamKind::Bool => ("bool", None),
            ParamKind::Texture => ("ref:image", Some("image")),
            ParamKind::Lut => ("ref:lut", Some("lut")),
        };
        let range = p.min.zip(p.max).map_or(Value::Null, |(a, b)| json!([a, b]));
        let source =
            SOURCES.iter().find(|s| current.is_some_and(|c| c.get(**s).is_some())).map_or("", |s| *s);
        self.out.push(Field {
            pointer: format!("{base}/{}", escape(&p.id)),
            key: p.id.clone(),
            widget: widget.into(),
            value: current.cloned().unwrap_or_default(),
            default: serde_json::to_value(&p.default).unwrap_or_default(),
            set: current.is_some(),
            options: reference.map(|r| candidates(self.doc, r, base)).unwrap_or_default(),
            range,
            bindable: reference.is_none(),
            source: source.into(),
            depth,
            ..Field::default()
        });
    }
}

fn tag(root: &Value, branch: &Value) -> Option<Value> {
    let (b, _) = deref(root, branch);
    b["enum"].get(0).or(b.get("const")).or(b["required"].get(0)).cloned()
}

fn tagged(root: &Value, schema: &Value) -> bool {
    let list = branches(schema);
    !list.is_empty() && list.iter().all(|b| tag(root, b).is_some())
}

fn array(root: &Value, meta: &Value, schema: &Value) -> String {
    let (items, _) = deref(root, schema.get("items").unwrap_or(&Value::Null));
    let fixed = schema["minItems"] == schema["maxItems"] && schema["minItems"].is_u64();
    let named = items.get("enum").is_some();
    let pair = items["minItems"] == 2 && items["maxItems"] == 2;
    let stop = items["prefixItems"].get(1).is_some_and(|p| deref(root, p).1 == "Color");
    match (fixed, schema["minItems"].as_u64(), items["type"].as_str()) {
        (true, Some(2), Some("number" | "integer")) => "vec2".into(),
        (true, Some(_), Some("number" | "integer")) => "numbers".into(),
        _ if meta["x-curve"] == true => "curve".into(),
        _ if named => "flags".into(),
        (_, _, Some("string")) => "lines".into(),
        (_, _, Some("array")) if stop => "stops".into(),
        (true, _, Some("array")) if pair => "points".into(),
        _ => "list".into(),
    }
}

pub fn fields_at(doc: &Value, pointer: &str, time: f64) -> Vec<Field> {
    let mut out = fields(doc, pointer);
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
    let (Some((root, _)), Some(schema)) = (root_of(pointer), at(doc, pointer)) else { return Vec::new() };
    let value = doc.pointer(pointer).cloned().unwrap_or_default();
    let top = hidden(doc);
    let (hidden, order): (&[&str], &str) = match pointer {
        "/manifest" => (&top, ORDER),
        _ if std::ptr::eq(root, &*MANIFEST) => (&[], ORDER),
        _ if std::ptr::eq(root, &*PROPERTY) => (&HIDDEN, crate::settings::ORDER),
        _ => (&HIDDEN, ""),
    };
    let mut walk = Walk { doc, root, hidden, order, out: Vec::new() };
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
    walk.out.iter_mut().for_each(|f| adjust(doc, f));
    walk.out
}
