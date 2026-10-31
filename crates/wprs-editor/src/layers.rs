use crate::starters::fold;
use anyhow::{Result, anyhow, bail};
use serde_json::{Value, json};
use std::path::Path;
use wprs_core::message::said;

pub const KINDS: [&str; 10] =
    ["image", "video", "text", "shape", "solid", "spectrum", "particles", "sound", "group", "adjustment"];
pub const LAYERS: &str = "/scene/layers";

pub fn walk(doc: &Value) -> Vec<(String, &Value)> {
    fn visit<'a>(list: &'a Value, base: &str, out: &mut Vec<(String, &'a Value)>) {
        for (i, layer) in list.as_array().into_iter().flatten().enumerate() {
            let path = format!("{base}/{i}");
            out.push((path.clone(), layer));
            visit(&layer["layers"], &format!("{path}/layers"), out);
        }
    }
    let mut out = Vec::new();
    visit(doc.pointer(LAYERS).unwrap_or(&Value::Null), LAYERS, &mut out);
    out
}

pub fn find(doc: &Value, id: &str) -> Result<String> {
    let found = walk(doc).into_iter().find(|(_, l)| l["id"] == id).map(|(path, _)| path);
    found.ok_or_else(|| anyhow!(said("editor.there_no_layer", "there is no layer \"%1\"", &[&id])))
}

pub fn parent(path: &str) -> (&str, usize) {
    let (list, i) = path.rsplit_once('/').unwrap_or((LAYERS, "0"));
    (list, i.parse().unwrap_or(0))
}

pub fn children(doc: &Value, group: Option<&str>) -> Result<String> {
    let Some(id) = group else { return Ok(LAYERS.into()) };
    let path = find(doc, id)?;
    match doc.pointer(&path).map(|l| &l["type"]) {
        Some(Value::String(kind)) if kind == "group" => Ok(format!("{path}/layers")),
        _ => bail!(said("editor.not_group", "%1 is not a group", &[&id])),
    }
}

pub fn taken(doc: &Value, id: &str) -> bool {
    let assets = doc.pointer("/scene/assets").and_then(|a| a.get(id)).is_some();
    let systems = doc.pointer("/scene/particles").and_then(|p| p.get(id)).is_some();
    assets || systems || walk(doc).iter().any(|(_, l)| l["id"] == id)
}

pub fn unique(doc: &Value, base: &str) -> String {
    unique_among(doc, base, &[])
}

pub fn clean(base: &str) -> String {
    let folded: String = base.to_lowercase().chars().map(fold).collect();
    let words: Vec<&str> = folded
        .split(|c: char| !c.is_ascii_lowercase() && !c.is_ascii_digit())
        .filter(|w| !w.is_empty())
        .collect();
    words.join("_").chars().take(26).collect::<String>().trim_end_matches('_').into()
}

fn unique_among(doc: &Value, base: &str, more: &[String]) -> String {
    let clean = clean(base);
    let base = if clean.is_empty() { "layer".into() } else { clean };
    let candidates = (1..).map(|n| if n == 1 { base.clone() } else { format!("{base}_{n}") });
    candidates.take(10_000).find(|id| !taken(doc, id) && !more.contains(id)).unwrap_or(base)
}

pub fn asset_kind(path: &str) -> &'static str {
    let ext = Path::new(path).extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" => "image",
        "webm" | "mp4" | "mkv" => "video",
        "ogg" | "opus" | "flac" | "wav" | "mp3" => "sound",
        "ttf" | "otf" | "ttc" | "otc" | "woff2" => "font",
        "cube" => "lut",
        _ => "other",
    }
}

pub fn assets_of<'a>(doc: &'a Value, kind: &str) -> Vec<&'a str> {
    let assets = doc.pointer("/scene/assets").and_then(Value::as_object).into_iter().flatten();
    assets
        .filter(|(_, a)| asset_kind(a["path"].as_str().unwrap_or_default()) == kind)
        .map(|(id, _)| id.as_str())
        .collect()
}

pub fn template(
    doc: &Value,
    kind: &str,
    id: &str,
    source: Option<&str>,
    name: Option<&str>,
) -> Result<Value> {
    let asset = |what: &str| {
        let found = source.map(String::from).or_else(|| assets_of(doc, what).first().map(|a| a.to_string()));
        let article = if what == "image" { "an" } else { "a" };
        found.ok_or_else(|| anyhow!(said("editor.import", "import %1 %2 first", &[&article, &what])))
    };
    Ok(match kind {
        "image" => json!({"id": id, "type": "image", "image": asset("image")?}),
        "video" => json!({"id": id, "type": "video", "video": asset("video")?}),
        "sound" => json!({"id": id, "type": "sound", "sound": asset("sound")?}),
        "text" => {
            json!({"id": id, "type": "text", "text": name.unwrap_or("Text"), "size": 96, "color": "#ffffff"})
        }
        "shape" => json!({"id": id, "type": "shape", "shape": {"rect": {"size": [480, 320], "radius": 24}},
            "fill": "#4a90d9"}),
        "solid" => json!({"id": id, "type": "solid", "color": "#203040"}),
        "spectrum" => json!({"id": id, "type": "spectrum", "size": [960, 240]}),
        "particles" => json!({"id": id, "type": "particles", "system": source.unwrap_or(id)}),
        "group" => json!({"id": id, "type": "group", "layers": []}),
        "adjustment" => json!({"id": id, "type": "adjustment", "effects": []}),
        _ => bail!(said(
            "editor.not_layer_type",
            "%1 is not a layer type; use one of %2",
            &[&kind, &(KINDS.join(", "))]
        )),
    })
}

pub fn system() -> Value {
    json!({"capacity": 2000, "emitters": [{"shape": "edge", "rate": 120}],
        "init": {"life": [3, 6], "speed": [20, 60], "size": [3, 8], "color": ["#ffffff", "#a0c8ff"]},
        "forces": [{"turbulence": {"strength": 20, "scale": 0.004}}]})
}

pub fn renamed(doc: &Value, layer: &Value, used: &mut Vec<String>) -> Value {
    let mut copy = layer.clone();
    let id = unique_among(doc, &format!("{}_copy", layer["id"].as_str().unwrap_or("layer")), used);
    used.push(id.clone());
    copy["id"] = json!(id);
    if let Some(list) = layer["layers"].as_array() {
        copy["layers"] = list.iter().map(|child| renamed(doc, child, used)).collect();
    }
    copy
}
