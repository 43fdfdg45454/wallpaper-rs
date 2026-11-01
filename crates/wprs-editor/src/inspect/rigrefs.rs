use super::{Field, Walk};
use crate::ops::escape;
use crate::schema::sample;
use serde_json::{Value, json};
use std::path::Path;

pub(super) const RIG: [&str; 5] = ["bone", "point", "part", "shape", "reach"];

fn owner(doc: &Value, pointer: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = pointer.split('/').collect();
    for i in (2..=parts.len()).rev() {
        let Some(at) = doc.pointer(&parts[..i].join("/")) else { continue };
        if parts[i - 1] == "attach" {
            return Some((at["layer"].as_str()?.to_owned(), String::new()));
        }
        if at["type"].is_string() && at["id"].is_string() {
            let part = (parts.get(i) == Some(&"parts")).then(|| parts.get(i + 1)).flatten();
            let part = part.map_or(String::new(), |p| p.replace("~1", "/").replace("~0", "~"));
            return (at["type"] == "mesh").then_some((at["id"].as_str()?.to_owned(), part));
        }
    }
    None
}

pub(super) fn options(
    dir: &Path,
    doc: &Value,
    reference: &str,
    pointer: &str,
    current: &Value,
) -> Option<Vec<Value>> {
    let (id, part) = owner(doc, pointer)?;
    let known = crate::rigs::known(dir, doc, &id);
    let list = |k: &str| known[k].as_array().cloned().unwrap_or_default();
    let found = match reference {
        "bone" => list("bones"),
        "point" => list("points"),
        "part" => list("parts").into_iter().filter(|p| *p != part.as_str()).collect(),
        "shape" => {
            let layer = crate::actions::layer(doc, &id).ok()?.1;
            let own = layer["expressions"].as_object().into_iter().flatten().map(|(k, _)| json!(k));
            list("shapes").into_iter().chain(own).collect()
        }
        _ => {
            let nodes = list("bones").into_iter().chain(list("points"));
            let bones = nodes.filter_map(|n| Some(json!(format!("bone:{}", n.as_str()?))));
            let layers =
                crate::layers::walk(doc).into_iter().filter_map(|(_, l)| l["id"].as_str().map(String::from));
            let layers = layers.filter(|l| *l != id).map(|l| json!(format!("layer:{l}")));
            let point = if current.is_array() { current.clone() } else { json!([0, 0]) };
            std::iter::once(json!("cursor")).chain(bones).chain(layers).chain([point]).collect()
        }
    };
    Some(found).filter(|f| !f.is_empty())
}

impl Walk<'_> {
    pub(super) fn rigged(&mut self, mut f: Field, r: &str, schema: &Value, current: &Value) {
        let (pointer, depth) = (f.pointer.clone(), f.depth);
        let kind = schema["type"].as_str().filter(|_| r != "reach").unwrap_or_default();
        f.widget = format!(
            "{}:{r}",
            match kind {
                "array" => "refs",
                "object" => "map",
                _ => "ref",
            }
        );
        let inner = schema.get("additionalProperties").cloned().unwrap_or_default();
        if kind == "object" {
            f.sample = sample(self.root, &inner);
        }
        self.out.push(f);
        if kind == "object" && depth < 3 {
            for (key, v) in current.as_object().into_iter().flatten() {
                self.field(&inner, Some(v), format!("{pointer}/{}", escape(key)), key, depth + 1);
            }
        }
    }
}
