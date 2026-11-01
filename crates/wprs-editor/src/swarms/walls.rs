use super::shapes::Frame;
use serde_json::{Value, json};
use std::path::Path;

fn pair(v: &Value) -> [f64; 2] {
    [0, 1].map(|i| v.get(i).and_then(Value::as_f64).unwrap_or(0.0))
}

pub fn response(c: &Value) -> &'static str {
    let (bounce, friction) = (c["bounce"].as_f64().unwrap_or(0.0), c["friction"].as_f64().unwrap_or(0.0));
    match () {
        _ if c["kill"] == true => "kill",
        _ if c["stick"] == true => "stick",
        _ if bounce > 0.0 => "bounce",
        _ if friction >= 1.0 => "stop",
        _ => "slide",
    }
}

fn layer(dir: &Path, doc: &Value, id: &str, t: f64) -> Value {
    let mut out = json!({ "id": id, "outline": [], "field": [], "approximate": true });
    let Some(g) = crate::canvas::gizmos(dir, doc, t).into_iter().find(|g| g.id == id) else { return out };
    let (s, c) = g.rotation.to_radians().sin_cos();
    let at = [g.offset[0] + g.position[0], g.offset[1] + g.position[1]];
    let [w, h] = [g.size[0] * g.scale[0], g.size[1] * g.scale[1]];
    let place = |u: f64, v: f64| {
        let [x, y] = [(u - 0.5) * w, (v - 0.5) * h];
        [((at[0] + x * c - y * s) * 100.0).round() / 100.0, ((at[1] + x * s + y * c) * 100.0).round() / 100.0]
    };
    out["outline"] = json!([place(0.0, 0.0), place(1.0, 0.0), place(1.0, 1.0), place(0.0, 1.0)]);
    let found = crate::actions::layer(doc, id).map(|(_, l)| l.clone()).unwrap_or_default();
    let asset = crate::keys::value_at(&found["image"], t);
    let path = asset.as_str().and_then(|a| doc["scene"]["assets"][a]["path"].as_str()).map(|p| dir.join(p));
    let Some(picture) = path.filter(|_| found["type"] == "image").and_then(|p| image::open(p).ok()) else {
        return out;
    };
    let small = picture.thumbnail(62, 62).to_rgba8();
    let (sw, sh) = (small.width() + 2, small.height() + 2);
    let mut rgba = vec![0u8; (sw * sh * 4) as usize];
    for (x, y, p) in small.enumerate_pixels() {
        rgba[(((y + 1) * sw + x + 1) * 4 + 3) as usize] = p.0[3];
    }
    let (cells, d) = wprs_scene::distance_field(sw, sh, &rgba, 0.5, [sw as f32, sh as f32]);
    let unit = w.max(h) / f64::from(small.width().max(small.height()));
    let step = cells[0].max(cells[1]).div_ceil(24).max(1);
    let field = (0..cells[1]).step_by(step).flat_map(|y| (0..cells[0]).step_by(step).map(move |x| (x, y)));
    let field = field.map(|(x, y)| {
        let (u, v) =
            ((x as f64 - 0.5) / f64::from(small.width()), (y as f64 - 0.5) / f64::from(small.height()));
        let [px, py] = place(u, v);
        json!([px, py, (f64::from(d[y * cells[0] + x]) * unit * 10.0).round() / 10.0])
    });
    out["field"] = json!(field.collect::<Vec<_>>());
    out["approximate"] = json!(false);
    out
}

pub(super) fn colliders(dir: &Path, doc: &Value, s: &Value, f: &Frame, t: f64) -> Vec<Value> {
    let canvas = pair(&doc["scene"]["canvas"]["size"]);
    let list = s["collide"].as_array().into_iter().flatten().enumerate();
    list.map(|(k, c)| {
        let b = &c["bounds"];
        let (kind, v) = b.as_object().and_then(|o| o.iter().next()).map_or((b.as_str().unwrap_or("canvas"), &Value::Null), |(k, v)| (k.as_str(), v));
        let p = |v: &Value| f.round(f.apply(pair(v)));
        let mut out = match kind {
            "plane" => {
                let (at, n) = (pair(&v["point"]), pair(&v["normal"]));
                let l = n[0].hypot(n[1]).max(1e-9);
                let tip = f.round(f.apply([at[0] + n[0] / l * 60.0, at[1] + n[1] / l * 60.0]));
                let along = [-n[1] / l * 4000.0, n[0] / l * 4000.0];
                let line = [p(&json!([at[0] - along[0], at[1] - along[1]])), p(&json!([at[0] + along[0], at[1] + along[1]]))];
                json!({ "point": p(&v["point"]), "tip": tip, "line": line })
            }
            "circle" => {
                let center = pair(&v["center"]);
                let radius = v["radius"].as_f64().unwrap_or(0.0);
                json!({ "center": p(&v["center"]), "radius": radius * f.scale(), "edge": p(&json!([center[0] + radius, center[1]])) })
            }
            "box" => {
                let (a, b) = (pair(&v["from"]), pair(&v["to"]));
                let corners = [a, [b[0], a[1]], b, [a[0], b[1]]].map(|q| p(&json!(q)));
                json!({ "corners": corners, "from": p(&v["from"]), "to": p(&v["to"]) })
            }
            "segment" => json!({ "from": p(&v["from"]), "to": p(&v["to"]) }),
            "layer" => layer(dir, doc, v["id"].as_str().unwrap_or_default(), t),
            "mask" => json!({ "texture": v["texture"] }),
            _ => json!({ "corners": [[0.0, 0.0], [canvas[0], 0.0], canvas, [0.0, canvas[1]]] }),
        };
        out["index"] = json!(k);
        out["kind"] = json!(kind);
        out["inside"] = json!(v["inside"] == true);
        out["response"] = json!(response(c));
        out["bounce"] = json!(c["bounce"].as_f64().unwrap_or(0.0));
        out["friction"] = json!(c["friction"].as_f64().unwrap_or(0.0));
        out["value"] = c.clone();
        out
    }).collect()
}
