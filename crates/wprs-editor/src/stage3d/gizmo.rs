use super::view::{View, camera};
use crate::rigs::round;
use serde_json::{Value, json};
use std::path::Path;
use std::sync::Arc;
use wprs_scene::{Affine, Eye, Quality, Runtime, Values, apply4};

const HANDLE: f32 = 70.0;

fn px(p: [f32; 2]) -> Value {
    json!(p.map(round))
}

struct Seen {
    eye: Eye,
    to: Affine,
    size: [f32; 2],
}

impl Seen {
    fn at(&self, p: [f32; 3]) -> Option<[f32; 2]> {
        let [x, y, _] = self.eye.project(p, self.size)?;
        Some(self.to.apply([x, y]))
    }
}

fn corners([lo, hi]: [[f32; 3]; 2]) -> impl Iterator<Item = [f32; 3]> {
    (0..8).map(move |k| [0, 1, 2].map(|i| if k >> i & 1 == 1 { hi[i] } else { lo[i] }))
}

fn world(m: &wprs_scene::Mat4, p: [f32; 3]) -> [f32; 3] {
    let w = apply4(m, p, 1.0);
    [w[0], w[1], w[2]]
}

pub(crate) fn framed(dir: &Path, doc: &Value, id: &str) -> Option<([f32; 3], f32)> {
    let scene = super::store::scene(dir, doc)?;
    let size = scene.canvas.size.map(|v| v as f32);
    let rt = Runtime::new(Arc::new(scene), Values::new());
    let frame = rt.frame([0.0, 0.0, size[0], size[1]], Quality::Q3);
    let p = frame.placed.iter().find(|p| p.layer.id == id)?;
    let m = &p.matrix;
    let k = (0..3)
        .map(|i| (m[i * 4] * m[i * 4] + m[i * 4 + 1] * m[i * 4 + 1] + m[i * 4 + 2] * m[i * 4 + 2]).sqrt());
    Some((world(m, p.model.middle()), p.model.radius() * k.fold(0.0, f32::max)))
}

fn frustum(seen: &Seen, cam: &Eye) -> Vec<Value> {
    let (r, u, f) = cam.basis();
    let far = cam.far.min(cam.distance() * 2.0);
    let t = (cam.fov.to_radians() * 0.5).tan();
    let ring = |d: f32| -> Vec<[f32; 3]> {
        let (h, w) = (t * d, t * d * cam.aspect);
        [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
            .map(|(a, b)| std::array::from_fn(|i| cam.at[i] + f[i] * d + r[i] * a * w + u[i] * b * h))
            .to_vec()
    };
    let (near, back) = (ring(cam.near.max(far * 0.02)), ring(far));
    let mut lines: Vec<([f32; 3], [f32; 3])> = (0..4).map(|k| (cam.at, back[k])).collect();
    lines.extend((0..4).flat_map(|k| [(near[k], near[(k + 1) % 4]), (back[k], back[(k + 1) % 4])]));
    let seg = |(a, b): ([f32; 3], [f32; 3])| Some(json!([px(seen.at(a)?), px(seen.at(b)?)]));
    lines.into_iter().filter_map(seg).collect()
}

fn model(doc: &Value, seen: &Seen, p: &wprs_scene::Placed, stage: &str) -> Option<Value> {
    let c = world(&p.matrix, p.model.middle());
    let at = seen.at(c)?;
    let depth = seen.eye.depth(c).max(seen.eye.near);
    let length = HANDLE * seen.eye.unit(seen.size[1]) * depth / seen.eye.distance();
    let arms: Vec<Value> = (0..3)
        .filter_map(|k| {
            let mut e = c;
            e[k] += length;
            seen.at(e).map(px)
        })
        .collect();
    let shown: Vec<[f32; 2]> = corners(p.model.bounds).filter_map(|q| seen.at(world(&p.matrix, q))).collect();
    let lo = shown.iter().fold([f32::MAX; 2], |m, q| [m[0].min(q[0]), m[1].min(q[1])]);
    let hi = shown.iter().fold([f32::MIN; 2], |m, q| [m[0].max(q[0]), m[1].max(q[1])]);
    let pointer = crate::actions::layer(doc, &p.layer.id).map(|(p, _)| p).unwrap_or_default();
    let layer = doc.pointer(&pointer).cloned().unwrap_or_default();
    Some(json!({ "id": p.layer.id, "pointer": pointer, "stage": stage, "at": px(at), "arms": arms,
        "length": round(length), "depth": round(depth), "box": [round(lo[0]), round(lo[1]), round(hi[0]), round(hi[1])],
        "position": layer["position"], "rotation": layer["rotation"], "scale": layer["scale"] }))
}

pub fn stage(dir: &Path, doc: &Value, t: f64) -> Value {
    let Some(scene) = super::store::scene(dir, doc) else { return Value::Null };
    let size = scene.canvas.size.map(|v| v as f32);
    let mut rt = Runtime::new(Arc::new(scene), Values::new());
    rt.advance(t.max(0.0));
    let frame = rt.frame([0.0, 0.0, size[0], size[1]], Quality::Q3);
    let raw = &doc["scene"];
    let kept = View::read(raw);
    let view =
        View { through: kept.is_none_or(|v| v.through), ..kept.unwrap_or_else(|| View::from(&camera(raw))) };
    let back = frame.camera.inverse();
    let (mut models, mut lines, mut handles) = (Vec::new(), Vec::new(), json!(null));
    for s in &frame.stages {
        let to =
            back.then(s.world).then(Affine::translate([-s.origin[0] * s.size[0], -s.origin[1] * s.size[1]]));
        let cam = s.eye;
        let eye = if view.through { cam } else { view.eye(&cam) };
        let seen = Seen { eye: Eye { aspect: cam.aspect, ..eye }, to, size: s.size };
        models.extend(frame.placed[s.models.clone()].iter().filter_map(|p| model(doc, &seen, p, s.layer)));
        if !view.through && handles.is_null() {
            lines = frustum(&seen, &cam);
            let up: [f32; 3] = std::array::from_fn(|i| cam.at[i] + cam.basis().1[i] * cam.distance() * 0.25);
            let (right, upward, _) = seen.eye.basis();
            handles = json!({ "eye": seen.at(cam.at).map(px), "center": seen.at(cam.center).map(px),
                "up": seen.at(up).map(px), "unit": round(seen.eye.unit(s.size[1])),
                "right": right.map(round), "upward": upward.map(round) });
        }
    }
    let groups: Vec<String> = crate::layers::walk(doc)
        .into_iter()
        .filter(|(_, l)| l["type"] == "group" && l["space"] == "3d")
        .map(|(p, _)| format!("{p}/layers/"))
        .collect();
    let direct =
        |p: &str| groups.iter().any(|g| p.strip_prefix(g.as_str()).is_some_and(|rest| !rest.contains('/')));
    let flat: Vec<Value> = crate::layers::walk(doc)
        .into_iter()
        .filter(|(p, l)| l["type"] != "model" && direct(p))
        .map(|(p, l)| json!({ "id": l["id"], "pointer": p, "depth_test": l["depth_test"] == true }))
        .collect();
    let c = camera(raw);
    let scene_camera = json!({ "eye": c.at.map(round), "center": c.center.map(round), "up": c.up.map(round), "fov": round(c.fov),
        "near": round(c.near), "far": round(c.far) });
    json!({ "size": size, "view": view.json(), "through": view.through, "camera": scene_camera, "models": models,
        "frustum": lines, "handles": handles, "flat": flat, "stages": frame.stages.len() })
}
