use super::{Field, Walk};
use crate::ops::escape;
use crate::schema::{branches, choose, deref, fits, properties, sample};
use serde_json::{Value, json};
use wprs_scene::described;

impl Walk<'_> {
    pub(super) fn variant(&mut self, mut f: Field, schema: &Value, current: &Value) {
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

    pub(super) fn object(&mut self, schema: &Value, value: &Value, pointer: &str, depth: usize) {
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
            if self.hidden.contains(&key.as_str()) && depth == 0
                || key == "ext"
                || !clocked(self.doc, pointer, &key)
            {
                continue;
            }
            let path = format!("{pointer}/{}", escape(&key));
            let found = |id: &str| self.own.get(id).map(|d| &**d).or_else(|| described(id));
            match (key.as_str(), value["effect"].as_str().and_then(found)) {
                ("params", Some(d)) => {
                    let params =
                        d.params.iter().map(|p| crate::params::param(self.doc, p, value, &path, depth));
                    let options = d.options.iter().map(|o| crate::params::option(o, value, &path, depth));
                    let wake = std::iter::once(crate::params::wake(self.doc, d, &path, depth));
                    self.out.extend(params.chain(options).chain(wake).collect::<Vec<_>>())
                }
                _ => self.field(&meta, value.get(&key), path, &key, depth),
            }
            if let Some(f) = self.out.get_mut(first) {
                f.required = required.contains(&&json!(key)) || meta["x-required"] == true;
            }
        }
    }
}

fn clocked(doc: &Value, pointer: &str, key: &str) -> bool {
    let layer = pointer.strip_suffix("/playback").and_then(|p| doc.pointer(p)).filter(|_| key == "own_clock");
    let pooled = |id: &str| doc["scene"]["pools"].get(id).is_some();
    let film = |a: &Value| {
        let path = a.as_str().and_then(|a| doc["scene"]["assets"][a]["path"].as_str()).unwrap_or_default();
        let ext = path.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
        wprs_scene::VIDEOS.contains(&ext.as_str())
    };
    let file = |p: &Value| {
        let prop = doc.pointer(&format!("/manifest/properties/{}", escape(p.as_str().unwrap_or_default())));
        let accepts = |p: &Value| p["accept"].as_array().is_some_and(|a| a.contains(&json!("video")));
        prop.is_some_and(|p| p["type"] == "file" && (accepts(p) || film(&p["default_asset"])))
    };
    let named = |l: &Value| match &l["video"] {
        Value::String(v) => !pooled(v),
        v if v.get("map").is_none() => file(&v["prop"]),
        v => v["map"].as_object().is_some_and(|m| m.values().filter_map(Value::as_str).any(|v| !pooled(v))),
    };
    layer.is_none_or(|l| l["type"] == "video" && named(l))
}

fn tag(root: &Value, branch: &Value) -> Option<Value> {
    let (b, _) = deref(root, branch);
    b["enum"].get(0).or(b.get("const")).or(b["required"].get(0)).cloned()
}

pub(super) fn tagged(root: &Value, schema: &Value) -> bool {
    let list = branches(schema);
    !list.is_empty() && list.iter().all(|b| tag(root, b).is_some())
}

pub(super) fn array(root: &Value, meta: &Value, schema: &Value) -> String {
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
