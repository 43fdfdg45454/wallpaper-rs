use serde_json::Value as Json;
use std::sync::LazyLock;

static SCHEMAS: LazyLock<Json> =
    LazyLock::new(|| serde_json::from_str(include_str!("released.json")).unwrap_or_default());

pub fn newer_scene_keys(scene: &Json) -> bool {
    newer_keys("wallpaper-rs.scene/1", scene)
}

pub fn newer_manifest_keys(manifest: &Json) -> bool {
    newer_keys("wallpaper-rs.wpak/2", manifest)
}

fn newer_keys(schema: &str, value: &Json) -> bool {
    let root = &SCHEMAS[schema];
    root.is_object() && newer(root, root, value)
}

fn deref<'a>(root: &'a Json, mut schema: &'a Json) -> &'a Json {
    while let Some(name) = schema["$ref"].as_str().and_then(|r| r.strip_prefix("#/$defs/")) {
        schema = &root["$defs"][name];
    }
    schema
}

fn branches(schema: &Json) -> impl Iterator<Item = &Json> {
    ["anyOf", "oneOf"].into_iter().flat_map(|k| schema[k].as_array().into_iter().flatten())
}

fn fits(root: &Json, schema: &Json, value: &Json) -> bool {
    let schema = deref(root, schema);
    let kind = |t: &Json| match t.as_str() {
        Some("object") => value.is_object(),
        Some("array") => value.is_array(),
        Some("string") => value.is_string(),
        Some("number") => value.is_number(),
        Some("integer") => value.is_i64() || value.is_u64(),
        Some("boolean") => value.is_boolean(),
        Some("null") => value.is_null(),
        _ => true,
    };
    let typed = schema["type"].as_array().map_or_else(|| kind(&schema["type"]), |t| t.iter().any(kind));
    let tags = schema["properties"].as_object().into_iter().flatten();
    let tagged =
        tags.filter_map(|(k, p)| p.get("const").map(|c| (k, c))).all(|(k, c)| value.get(k) == Some(c));
    let required = schema["required"].as_array().into_iter().flatten().filter_map(Json::as_str);
    let present = !value.is_object() || required.into_iter().all(|k| value.get(k).is_some());
    let constant = schema.get("const").is_none_or(|c| c == value);
    let mut nested = branches(schema).peekable();
    let branch = nested.peek().is_none() || nested.any(|b| fits(root, b, value));
    typed && tagged && present && constant && branch
}

fn options<'a>(root: &'a Json, schema: &'a Json, value: &Json) -> Vec<Vec<&'a Json>> {
    let schema = deref(root, schema);
    if branches(schema).next().is_none() {
        return vec![vec![schema]];
    }
    let fitting = branches(schema).filter(|b| fits(root, b, value));
    let nested = fitting.flat_map(|b| options(root, b, value));
    nested.map(|parts| parts.into_iter().chain([schema]).collect()).collect()
}

fn newer(root: &Json, schema: &Json, value: &Json) -> bool {
    let options = options(root, schema, value);
    !options.is_empty() && options.iter().all(|parts| within(root, parts, value))
}

fn within(root: &Json, parts: &[&Json], value: &Json) -> bool {
    match value {
        Json::Object(map) => map.iter().any(|(key, value)| {
            let known = parts.iter().find_map(|p| p["properties"].get(key));
            let extra = parts.iter().map(|p| &p["additionalProperties"]).find(|a| a.is_object());
            let open = parts.iter().any(|p| p["additionalProperties"] == true);
            let closed =
                parts.iter().any(|p| p.get("properties").is_some() || p["additionalProperties"] == false);
            match known.or(extra) {
                Some(schema) => newer(root, schema, value),
                None => closed && !open,
            }
        }),
        Json::Array(items) => items.iter().enumerate().any(|(i, value)| {
            let item = |p: &Json| p["prefixItems"].get(i).is_some() || p["items"].is_object();
            let schema =
                parts.iter().find(|p| item(p)).map(|p| p["prefixItems"].get(i).unwrap_or(&p["items"]));
            schema.is_some_and(|schema| newer(root, schema, value))
        }),
        _ => false,
    }
}
