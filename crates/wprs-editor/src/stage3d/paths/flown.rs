use super::super::gizmo::{px, seen_for};
use crate::rigs::round;
use serde_json::{Value, json};
use std::path::Path;
use std::sync::Arc;
use wprs_scene::{Camera3d, Eye, Quality, Runtime, Values, path_period, path_pose, play_paths};

const SAMPLES: usize = 64;

pub fn paths(dir: &Path, doc: &Value, t: f64) -> Value {
    let Some(scene) = super::super::store::scene(dir, doc) else { return Value::Null };
    let size = scene.canvas.size.map(|v| v as f32);
    let cam: Camera3d = scene.camera3d.clone().unwrap_or_default();
    let mut raw = doc.clone();
    if let Some(c) = raw.pointer_mut("/scene/camera3d").and_then(Value::as_object_mut) {
        c.remove("paths");
    }
    let Some(still) = super::super::store::scene(dir, &raw) else { return Value::Null };
    let rt = Runtime::new(Arc::new(still), Values::new());
    let frame = rt.frame([0.0, 0.0, size[0], size[1]], Quality::Q3);
    let seen = seen_for(doc, &frame);
    let at = |p: [f32; 3]| seen.as_ref().and_then(|s| s.at(p)).map(px);
    let reach = |p: [f32; 3]| {
        let s = seen.as_ref()?;
        Some(round(s.eye.unit(s.size[1]) * s.eye.depth(p).max(s.eye.near) / s.eye.distance()))
    };
    let fov = scene.camera3d.as_ref().map_or(60.0, |c| match &c.fov {
        wprs_scene::Value::Fixed(f) => *f,
        _ => 60.0,
    });
    let base =
        Eye { at: [0.0; 3], center: [0.0; 3], up: cam.up.0, fov, near: cam.near, far: cam.far, aspect: 1.0 };
    let list: Vec<Value> = cam
        .paths
        .iter()
        .map(|p| {
            let period = path_period(p);
            let first = p.keys.first().map_or(0.0, |k| k.t);
            let keys: Vec<Value> = p
                .keys
                .iter()
                .map(|k| json!({ "t": k.t, "eye": k.eye.0.map(round), "center": k.center.0.map(round), "fov": k.fov,
                    "at": at(k.eye.0), "aim": at(k.center.0), "reach_eye": reach(k.eye.0), "reach_center": reach(k.center.0) }))
                .collect();
            let span = if p.closed { period } else { period - first };
            let sample = |k: usize| path_pose(p, first + span * k as f32 / SAMPLES as f32, base);
            let line: Vec<Value> = (0..=SAMPLES).filter_map(|k| at(sample(k).at)).collect();
            let aims: Vec<Value> = (0..=SAMPLES).step_by(4).filter_map(|k| at(sample(k).center)).collect();
            json!({ "id": p.id, "loop": p.closed, "ease": p.ease, "period": round(period), "keys": keys,
                "spline": line, "aims": aims })
        })
        .collect();
    let now = play_paths(&cam.paths, &cam, t as f32, fov);
    let playing = now.map(|f| {
        json!({ "eye": f.eye.at.map(round), "center": f.eye.center.map(round), "fov": round(f.eye.fov),
        "fade": round(f.fade), "cross": f.cross.map(|c| round(c.1)), "moving": f.moving, "at": at(f.eye.at) })
    });
    let total: f32 = cam.paths.iter().map(path_period).sum();
    json!({ "paths": list, "order": cam.order, "fade": cam.fade.map(|f| json!({ "to": f.to, "duration": f.duration })),
        "time": t, "total": round(total), "playing": playing })
}
