use serde_json::{Value, json};
use std::path::Path;

pub(super) fn samples(dir: &Path, doc: &Value, v: &Value, t: f64) -> Value {
    let id = v["id"].as_str().unwrap_or_default();
    let threshold = v["threshold"].as_f64().unwrap_or(0.5);
    let mut out = json!({ "kind": "layer", "id": id, "threshold": threshold, "color": v["color"], "inherit": v["inherit"],
        "jitter": v["jitter"], "refresh": v.get("refresh").unwrap_or(&json!("static")), "samples": [], "outline": [] });
    let Some(g) = crate::canvas::gizmos(dir, doc, t).into_iter().find(|g| g.id == id) else { return out };
    let (s, c) = g.rotation.to_radians().sin_cos();
    let at = [g.offset[0] + g.position[0], g.offset[1] + g.position[1]];
    let [w, h] = [g.size[0] * g.scale[0], g.size[1] * g.scale[1]];
    let place = |u: f64, v: f64| {
        let [x, y] = [(u - 0.5) * w, (v - 0.5) * h];
        [((at[0] + x * c - y * s) * 100.0).round() / 100.0, ((at[1] + x * s + y * c) * 100.0).round() / 100.0]
    };
    out["outline"] = json!([place(0.0, 0.0), place(1.0, 0.0), place(1.0, 1.0), place(0.0, 1.0)]);
    let layer = crate::actions::layer(doc, id).map(|(_, l)| l.clone()).unwrap_or_default();
    let asset = crate::keys::value_at(&layer["image"], t);
    let path = asset.as_str().and_then(|a| doc["scene"]["assets"][a]["path"].as_str()).map(|p| dir.join(p));
    let Some(picture) = path.filter(|_| layer["type"] == "image").and_then(|p| image::open(p).ok()) else {
        out["approximate"] = json!(true);
        return out;
    };
    let small = picture.thumbnail(48, 48).to_rgba8();
    let (sw, sh) = (f64::from(small.width()), f64::from(small.height()));
    let solid = small.enumerate_pixels().filter(|(_, _, p)| f64::from(p.0[3]) / 255.0 > threshold);
    out["samples"] = json!(
        solid
            .map(|(x, y, _)| place((f64::from(x) + 0.5) / sw, (f64::from(y) + 0.5) / sh))
            .collect::<Vec<_>>()
    );
    out
}
