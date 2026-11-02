use super::gizmo::{Seen, seen_at, world};
use super::turns::{M3, angles, apply, around, euler, flip, invert, mul, of_mat, of_mat4, pure};
use serde_json::{Value, json};
use std::path::Path;
use std::sync::Arc;
use wprs_scene::{Placed, Quality, Runtime, Values};

fn three(v: &Value) -> Option<[f32; 3]> {
    Some([v.get(0)?.as_f64()? as f32, v.get(1)?.as_f64()? as f32, v.get(2)?.as_f64()? as f32])
}

fn sampled(clip: &Value, bone: &str, channel: &str, t: f64) -> [f32; 3] {
    let keys = clip["bones"][bone][channel].as_array().cloned().unwrap_or_default();
    let at = t + clip["from"].as_f64().unwrap_or(0.0);
    let pick = |k: &Value| Some((k[0].as_f64()?, three(&k[1])?));
    let keys: Vec<(f64, [f32; 3])> = keys.iter().filter_map(pick).collect();
    let after = keys.iter().position(|(kt, _)| *kt > at);
    match after {
        None => keys.last().map_or([0.0; 3], |k| k.1),
        Some(0) => keys[0].1,
        Some(i) => {
            let ((t0, a), (t1, b)) = (keys[i - 1], keys[i]);
            let u = ((at - t0) / (t1 - t0).max(1e-9)) as f32;
            std::array::from_fn(|k| a[k] + (b[k] - a[k]) * u)
        }
    }
}

fn frames(p: &Placed, worlds: &[wprs_scene::Mat], n: usize, parent: Option<usize>) -> (M3, M3) {
    let model = of_mat4(&p.matrix);
    let bone = mul(&model, &of_mat(&worlds[n]));
    let up = parent.map_or(model, |q| mul(&model, &of_mat(&worlds[q])));
    (pure(&bone), up)
}

pub fn bone(dir: &Path, doc: &Value, q: &Value) -> Value {
    let (layer, name) = (q["layer"].as_str().unwrap_or_default(), q["bone"].as_str().unwrap_or_default());
    let Some(scene) = super::store::scene(dir, doc) else { return Value::Null };
    let size = scene.canvas.size.map(|v| v as f32);
    let mut rt = Runtime::new(Arc::new(scene), Values::new());
    let time = q["time"].as_f64().unwrap_or(0.0);
    rt.advance(time.max(0.0));
    let frame = rt.frame([0.0, 0.0, size[0], size[1]], Quality::Q3);
    let Some(p) = frame.placed.iter().find(|p| p.layer.id == layer) else { return Value::Null };
    let (Some(rig), Some(puppet)) = (p.model.rig.as_ref(), rt.puppet(layer)) else { return Value::Null };
    let Some(n) = rig.joint(name) else { return Value::Null };
    let Some(seen) = seen_at(doc, &frame, p.stage) else { return Value::Null };
    let worlds = puppet.joint_worlds();
    let parent = rig.joints[n].parent.map(usize::from);
    let (b, up) = frames(p, worlds, n, parent);
    let l = crate::actions::layer(doc, layer).map(|(_, l)| l.clone()).unwrap_or_default();
    let clip = crate::clips::found(&l, q["clip"].as_str().unwrap_or_default()).map(|(_, c)| c.clone());
    let t = q["clip_time"].as_f64().unwrap_or(time);
    let held = |k: &str| match three(&q["from"][k]) {
        Some(v) => v,
        None => clip.as_ref().map_or([0.0; 3], |c| sampled(c, name, k, t)),
    };
    let (start, shift) = (held("angles"), held("shift"));
    json!({ "angles": turned(&seen, &b, start, q), "shift": slid(&seen, p, worlds, (n, &up), shift, q) })
}

fn turned(seen: &Seen, b: &M3, start: [f32; 3], q: &Value) -> [f32; 3] {
    let (r, u, f) = seen.eye.basis();
    let n = |v: &Value| v.as_f64().unwrap_or(0.0) as f32;
    let tilt = &q["tilt"];
    let world = mul(&around(f, -n(&q["turn"])), &mul(&around(u, n(&tilt[0])), &around(r, n(&tilt[1]))));
    angles(&mul(&euler(start), &mul(&flip(b), &mul(&world, b))))
}

fn slid(
    seen: &Seen,
    p: &Placed,
    worlds: &[wprs_scene::Mat],
    (n, up): (usize, &M3),
    start: [f32; 3],
    q: &Value,
) -> [f32; 3] {
    let (r, u, _) = seen.eye.basis();
    let d = |k: usize| q["slide"][k].as_f64().unwrap_or(0.0) as f32;
    let w = &worlds[n];
    let head = world(&p.matrix, [w[3], w[7], w[11]]);
    let k = seen.eye.unit(seen.size[1]) * seen.eye.depth(head).max(seen.eye.near) / seen.eye.distance();
    let moved: [f32; 3] = std::array::from_fn(|i| (r[i] * d(0) - u[i] * d(1)) * k);
    let local = invert(up).map_or([0.0; 3], |inv| apply(&inv, moved));
    std::array::from_fn(|i| ((start[i] + local[i]) * 1000.0).round() / 1000.0)
}
