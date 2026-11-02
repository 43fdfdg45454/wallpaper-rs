use super::placed::{Placed, placed};
use crate::ops::{Op, escape, set};
use crate::rigs::round;
use anyhow::{Result, ensure};
use serde_json::{Value, json};
use std::path::Path;
use wprs_core::message::said;
use wprs_scene::{Beam, IMAGES, Scene, VIDEOS, projectable};

fn kind(doc: &Value, id: &str) -> Option<&'static str> {
    let path = doc.pointer(&format!("/scene/assets/{}/path", escape(id))).and_then(Value::as_str)?;
    let ext = path.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase())?;
    match () {
        _ if IMAGES.contains(&ext.as_str()) => Some("picture"),
        _ if VIDEOS.contains(&ext.as_str()) => Some("video"),
        _ => None,
    }
}

fn layers(doc: &Value) -> Vec<String> {
    let Ok(scene) = Scene::from_json(&doc["scene"].to_string()) else { return Vec::new() };
    let all = scene.layers().filter(|l| projectable(&scene, &l.id));
    all.map(|l| l.id.clone()).collect()
}

pub(crate) fn projection(
    _dir: &Path,
    doc: &Value,
    (path, beam): (&str, Beam),
    (asset, from): (Option<&str>, Option<&str>),
    off: bool,
) -> Result<Vec<Op>> {
    let at = format!("{path}/texture");
    if off {
        return Ok(doc.pointer(&at).map(|_| Op::Remove { path: at.clone() }).into_iter().collect());
    }
    ensure!(beam == Beam::Spot, said("editor.light_field", "only %1 lights take %2", &[&"spot", &"texture"]));
    match (asset, from) {
        (Some(id), None) => {
            ensure!(
                kind(doc, id).is_some(),
                said("editor.light_cast", "%1 is not a picture or video of the scene", &[&id])
            );
            Ok(set(doc, &at, json!(id)))
        }
        (None, Some(id)) => {
            ensure!(
                layers(doc).iter().any(|l| l == id),
                said(
                    "editor.light_cast_layer",
                    "the layer %1 cannot be projected: it is missing, takes light or reads a lit layer",
                    &[&id]
                )
            );
            Ok(set(doc, &at, json!({ "layer": id })))
        }
        _ => anyhow::bail!(said(
            "editor.light_cast_what",
            "give a picture, a video or a layer to project",
            &[]
        )),
    }
}

fn hit(l: &Placed, u: f32, v: f32) -> Option<[f32; 2]> {
    let a = l.heading;
    let level = a[0].hypot(a[1]) > 1e-4;
    let n = a[0].hypot(a[1]).max(1e-6);
    let side = if level { [a[1] / n, -a[0] / n, 0.0] } else { [1.0, 0.0, 0.0] };
    let up =
        [side[1] * a[2] - side[2] * a[1], side[2] * a[0] - side[0] * a[2], side[0] * a[1] - side[1] * a[0]];
    let c = (l.cone[1] * 0.5).to_radians().cos().min((l.cone[0] * 0.5).to_radians().cos() - 1e-4);
    let wide = (1.0 - c * c).max(1e-6).sqrt() / c.max(1e-3);
    let (s, t) = ((2.0 * u - 1.0) * wide, (2.0 * v - 1.0) * wide);
    let w: [f32; 3] = std::array::from_fn(|k| a[k] + s * side[k] + t * up[k]);
    let at = l.at();
    (w[2] < -1e-4 && l.z > 0.0).then(|| [at[0] - l.z / w[2] * w[0], at[1] - l.z / w[2] * w[1]])
}

pub fn homography(q: [[f32; 2]; 4]) -> [f32; 16] {
    let [p0, p1, p2, p3] = q;
    let (d1, d2) = ([p1[0] - p2[0], p1[1] - p2[1]], [p3[0] - p2[0], p3[1] - p2[1]]);
    let s = [p0[0] - p1[0] + p2[0] - p3[0], p0[1] - p1[1] + p2[1] - p3[1]];
    let det = (d1[0] * d2[1] - d2[0] * d1[1]).max(1e-9).copysign(d1[0] * d2[1] - d2[0] * d1[1]);
    let g = (s[0] * d2[1] - d2[0] * s[1]) / det;
    let h = (d1[0] * s[1] - s[0] * d1[1]) / det;
    let (a, b, c) = (p1[0] - p0[0] + g * p1[0], p3[0] - p0[0] + h * p3[0], p0[0]);
    let (d, e, f) = (p1[1] - p0[1] + g * p1[1], p3[1] - p0[1] + h * p3[1], p0[1]);
    [a, b, 0.0, c, d, e, 0.0, f, 0.0, 0.0, 1.0, 0.0, g, h, 0.0, 1.0]
}

fn thumb(dir: &Path, doc: &Value, id: &str) -> Option<String> {
    let path = doc.pointer(&format!("/scene/assets/{}/path", escape(id))).and_then(Value::as_str)?;
    let image = image::open(dir.join(path)).ok()?.thumbnail(128, 128).into_rgba8();
    Some(crate::rigs::url(&image))
}

pub fn cast(dir: &Path, doc: &Value, id: &str, t: f64) -> Value {
    let Ok((_, layer)) = crate::actions::layer(doc, id) else { return Value::Null };
    let Some(l) = placed(dir, doc, t).into_iter().find(|l| l.id == id) else { return Value::Null };
    let texture = &layer["texture"];
    let assets = doc["scene"]["assets"].as_object().into_iter().flatten().map(|(k, _)| k.as_str());
    let (mut pictures, mut videos) = (Vec::new(), Vec::new());
    for a in assets {
        match kind(doc, a) {
            Some("picture") => pictures.push(a),
            Some(_) => videos.push(a),
            None => {}
        }
    }
    let corners = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]].map(|[u, v]| hit(&l, u, v));
    let quad = corners.iter().all(Option::is_some).then(|| corners.map(|c| c.unwrap_or_default()));
    let shown = texture.as_str().filter(|a| kind(doc, a) == Some("picture"));
    json!({ "spot": l.beam == Beam::Spot, "current": texture, "pictures": pictures, "videos": videos,
        "layers": layers(doc), "quad": quad.map(|q| q.map(|p| p.map(round))),
        "matrix": quad.map(homography), "image": shown.and_then(|a| thumb(dir, doc, a)),
        "kind": texture.as_str().and_then(|a| kind(doc, a)).or(texture.get("layer").map(|_| "layer")) })
}
