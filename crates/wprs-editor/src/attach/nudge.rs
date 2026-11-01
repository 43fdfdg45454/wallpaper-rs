use crate::clips::num;
use crate::skin::Skin;
use anyhow::{Result, anyhow, ensure};
use serde_json::{Value, json};
use std::path::Path;
use wprs_core::message::said;

pub(crate) fn frame(skin: &Skin, node: usize, inherit: &Value) -> [f32; 4] {
    let has = |f: &str| inherit.as_array().is_none_or(|l| l.iter().any(|x| x == f));
    let w = skin.worlds()[node];
    let sx = w[0].hypot(w[4]).max(1e-6);
    let sy = (w[0] * w[5] - w[1] * w[4]) / sx;
    let (s, c) = if has("rotation") { (-w[4]).atan2(w[0]).sin_cos() } else { (0.0, 1.0) };
    let (sx, sy) = if has("scale") { (sx, sy) } else { (1.0, 1.0) };
    [c * sx, -s * sy, s * sx, c * sy]
}

pub(super) fn by(dir: &Path, doc: &Value, mesh: &Value, out: &mut Value, by: [f32; 2]) -> Result<()> {
    let bad = || anyhow!(said("editor.attach_nudge", "a layer is moved by two numbers of pixels", &[]));
    ensure!(by.iter().all(|v| v.is_finite()), bad());
    let [a, b, c, d] = match mesh["type"] == "mesh" {
        true => {
            let (_, skin) = Skin::load(dir, doc, mesh)?;
            let name = out["bone"].as_str().or(out["point"].as_str()).unwrap_or_default();
            let node = skin.names.iter().position(|n| n == name);
            let node =
                node.ok_or_else(|| anyhow!(said("editor.no_bone", "there is no bone \"%1\"", &[&name])))?;
            frame(&skin, node, &out["inherit"])
        }
        false => [1.0, 0.0, 0.0, 1.0],
    };
    let det = a * d - b * c;
    ensure!(det.abs() > 1e-9, bad());
    let [x, y] = by;
    let old = [0, 1].map(|i| out["offset"][i].as_f64().unwrap_or(0.0) as f32);
    let moved = [old[0] + (d * x - b * y) / det, old[1] + (a * y - c * x) / det];
    ensure!(moved.iter().all(|v| v.abs() <= 1e5), bad());
    out["offset"] = json!(moved.map(num));
    Ok(())
}
