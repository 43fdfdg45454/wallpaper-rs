use crate::actions::locked;
use crate::keys::value_at;
use serde::Serialize;
use serde_json::Value;
use std::path::Path;

static NULL: Value = Value::Null;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Gizmo {
    pub id: String,
    pub pointer: String,
    pub offset: [f64; 2],
    pub position: [f64; 2],
    pub size: [f64; 2],
    pub rotation: f64,
    pub scale: [f64; 2],
    pub locked: bool,
    pub kind: String,
}

fn pair(value: &Value, fallback: [f64; 2]) -> [f64; 2] {
    match value {
        Value::Number(n) => [n.as_f64().unwrap_or(0.0); 2],
        Value::Array(list) if list.len() == 2 => [0, 1].map(|i| list[i].as_f64().unwrap_or(fallback[i])),
        _ => fallback,
    }
}

fn size(dir: &Path, doc: &Value, layer: &Value, canvas: [f64; 2], t: f64) -> [f64; 2] {
    if layer.get("size").is_some_and(Value::is_array) {
        return pair(&layer["size"], canvas);
    }
    let file =
        |id: &Value| doc.pointer("/scene/assets")?.get(id.as_str()?)?["path"].as_str().map(|p| dir.join(p));
    match layer["type"].as_str().unwrap_or_default() {
        "image" => file(&value_at(&layer["image"], t))
            .and_then(|f| image::image_dimensions(f).ok())
            .map_or([512.0; 2], |(w, h)| [f64::from(w), f64::from(h)]),
        "video" => [640.0, 360.0],
        "text" => {
            let text = value_at(&layer["text"], t);
            let chars = text.as_str().or(layer["text"]["clock"].as_str()).map_or(4, |s| s.chars().count());
            let em = layer["size"].as_f64().unwrap_or(48.0);
            [em * 0.6 * chars as f64, em * 1.2]
        }
        "shape" => {
            let first = layer["shape"].as_object().and_then(|o| o.iter().next());
            let (kind, shape) = first.map_or(("", &NULL), |(k, v)| (k.as_str(), v));
            match kind {
                "polygon" | "star" => [shape["size"].as_f64().unwrap_or(100.0) * 2.0; 2],
                "line" => {
                    pair(&shape["to"], [100.0, 0.0]).map(|v| v.abs() + shape["width"].as_f64().unwrap_or(1.0))
                }
                _ => pair(&shape["size"], [100.0; 2]),
            }
        }
        _ => canvas,
    }
}

pub fn gizmos(dir: &Path, doc: &Value, t: f64) -> Vec<Gizmo> {
    let canvas = pair(&doc.pointer("/scene/canvas/size").cloned().unwrap_or_default(), [1920.0, 1080.0]);
    let locked = locked(doc);
    let mut out = Vec::new();
    let mut stack: Vec<(String, [f64; 2])> = vec![("/scene/layers".into(), [0.0; 2])];
    while let Some((list, parent)) = stack.pop() {
        for (i, layer) in doc.pointer(&list).and_then(Value::as_array).into_iter().flatten().enumerate() {
            let kind = layer["type"].as_str().unwrap_or_default().to_owned();
            if kind == "sound" {
                continue;
            }
            let anchor = layer["anchor"].as_str().and_then(|a| {
                let names = [
                    "top_left",
                    "top",
                    "top_right",
                    "left",
                    "center",
                    "right",
                    "bottom_left",
                    "bottom",
                    "bottom_right",
                ];
                names.iter().position(|n| *n == a)
            });
            let base = anchor
                .map_or([0.0; 2], |i| [(i % 3) as f64 / 2.0 * canvas[0], (i / 3) as f64 / 2.0 * canvas[1]]);
            let free = anchor.is_some() || matches!(kind.as_str(), "group" | "adjustment");
            let center = if free { [0.0; 2] } else { [canvas[0] / 2.0, canvas[1] / 2.0] };
            let transform = &layer["transform"];
            let position = pair(&value_at(&transform["position"], t), center);
            let offset = [parent[0] + base[0], parent[1] + base[1]];
            let pointer = format!("{list}/{i}");
            let id = layer["id"].as_str().unwrap_or_default().to_owned();
            if kind == "group" {
                stack.push((format!("{pointer}/layers"), [offset[0] + position[0], offset[1] + position[1]]));
            }
            out.push(Gizmo {
                locked: locked.contains(&id),
                size: size(dir, doc, layer, canvas, t),
                rotation: value_at(&transform["rotation"], t).as_f64().unwrap_or(0.0),
                scale: pair(&value_at(&transform["scale"], t), [1.0; 2]),
                id,
                pointer,
                offset,
                position,
                kind,
            });
        }
    }
    out
}
