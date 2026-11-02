use crate::rigs::round;
use serde_json::{Value, json};
use std::path::Path;
use wprs_scene::{Rigged, Track};

const STEPS: usize = 32;

pub fn root_path(rig: &Rigged, clip: &Value, laps: f32) -> Vec<[f32; 3]> {
    let motion = rig.motion(clip["animation"].as_str());
    let Some(motion) = motion.filter(|_| clip["animation"].is_string() || clip.get("bones").is_none()) else {
        return Vec::new();
    };
    let moves = |n: u16| motion.channels.iter().find(|c| c.node == n && c.path == Track::Translation);
    let Some(ch) = rig.order.iter().find_map(|n| moves(*n)) else { return Vec::new() };
    let from = clip["from"].as_f64().unwrap_or(0.0) as f32;
    let to = clip["to"].as_f64().map_or(motion.length, |t| t as f32);
    let len = (to - from).max(1e-3);
    let mut out = [0.0f32; 4];
    let mut at = |t: f32| {
        ch.sample(from + t, &mut out);
        [out[0], out[1], out[2]]
    };
    let (start, end) = (at(0.0), at(len));
    (0..=STEPS)
        .map(|k| {
            let t = k as f32 / STEPS as f32 * len * laps;
            let (lap, local) = ((t / len).floor(), t.rem_euclid(len));
            let now = at(if k == STEPS && local < 1e-4 { len } else { local });
            let lap = if k == STEPS && local < 1e-4 { lap - 1.0 } else { lap };
            [0, 1, 2].map(|i| if i == 1 { 0.0 } else { now[i] - start[i] + lap * (end[i] - start[i]) })
        })
        .collect()
}

pub fn motion(dir: &Path, doc: &Value, id: &str) -> Value {
    let Ok((pointer, l)) = super::figure(doc, id) else { return Value::Null };
    let Ok((_, _, rig)) = crate::skin::rigged(dir, doc, l) else { return Value::Null };
    let clips: Vec<Value> = l["clips"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|c| {
            let file = rig.motion(c["animation"].as_str()).map_or(0.0, |m| m.length);
            let from = c["from"].as_f64().unwrap_or(0.0);
            let to = c["to"].as_f64().unwrap_or(f64::from(file));
            let path: Vec<Value> = root_path(&rig, c, 2.0).iter().map(|p| json!(p.map(round))).collect();
            json!({ "id": c["id"], "animation": c["animation"], "from": from, "to": to, "length": round((to - from).max(0.0) as f32),
                "loop": c["loop"].as_str().unwrap_or("repeat"), "events": c["events"].as_array().cloned().unwrap_or_default(),
                "bones": c["bones"].as_object().map_or(0, |b| b.len()), "shapes": c["shapes"].as_object().map_or(0, |s| s.len()),
                "path": path })
        })
        .collect();
    let files: Vec<Value> =
        rig.motions.iter().map(|m| json!({ "name": m.name, "length": round(m.length) })).collect();
    let plays: Vec<Value> = l["animations"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(i, a)| {
            json!({ "index": i, "clip": a["clip"], "weight": a.get("weight").cloned().unwrap_or(json!(1)),
                "rate": a.get("rate").cloned().unwrap_or(json!(1)), "additive": a["additive"] == true,
                "blend_in": a["blend_in"].as_f64().unwrap_or(0.0), "blend_out": a["blend_out"].as_f64().unwrap_or(0.0),
                "enabled": a["enabled"] != false, "bones": a["bones"].as_array().cloned().unwrap_or_default() })
        })
        .collect();
    let bones: Vec<Value> = rig
        .joints
        .iter()
        .map(|j| json!({ "name": j.name, "parent": j.parent.map(|p| rig.joints[p as usize].name.clone()), "point": j.point }))
        .collect();
    let presets: Vec<&str> = super::PRESETS.iter().map(|p| p.0).collect();
    json!({ "layer": id, "pointer": pointer, "model": l["type"] == "model", "clips": clips, "files": files,
        "layers": plays, "bones": bones, "shapes": rig.shapes, "defaults": rig.defaults,
        "weights": l["shapes"].clone(), "root_motion": l["root_motion"] == true, "physics": l["physics"].clone(),
        "presets": presets })
}
