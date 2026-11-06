use super::looks;
use super::shapes::{Frame, shapes};
use crate::keys::value_at;
use crate::ops::escape;
use serde_json::{Value, json};
use std::path::Path;

fn pair(v: &Value) -> Option<[f64; 2]> {
    let list = v.as_array().filter(|l| l.len() == 2)?;
    Some([list[0].as_f64()?, list[1].as_f64()?])
}

pub(super) fn frame(dir: &Path, doc: &Value, layer: Option<&str>, t: f64) -> Frame {
    let canvas = pair(&doc["scene"]["canvas"]["size"]).unwrap_or([1920.0, 1080.0]);
    let found = layer.and_then(|id| crate::canvas::gizmos(dir, doc, t).into_iter().find(|g| g.id == id));
    let Some(g) = found else { return Frame::new([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]) };
    let at = [g.offset[0] + g.position[0], g.offset[1] + g.position[1]];
    let (s, c) = g.rotation.to_radians().sin_cos();
    let [sx, sy] = g.scale;
    let m = [c * sx, s * sx, -s * sy, c * sy];
    let [hx, hy] = [canvas[0] / 2.0, canvas[1] / 2.0];
    Frame::new([m[0], m[1], m[2], m[3], at[0] - m[0] * hx - m[2] * hy, at[1] - m[1] * hx - m[3] * hy])
}

fn points(s: &Value, f: &Frame, t: f64) -> Vec<Value> {
    let list = s["points"].as_array().cloned().unwrap_or_default();
    (0..list.len().max(1))
        .map(|k| {
            let p = list.get(k).cloned().unwrap_or(json!({}));
            let follow = p.get("follow").is_some();
            let local = pair(&value_at(&p["at"], t)).unwrap_or([0.0; 2]);
            let world = p["space"] == "world";
            let at = if world { local } else { f.apply(local) };
            let angle = value_at(&p["angle"], t).as_f64().unwrap_or(0.0) + f.turn();
            let bound = p["at"].is_object() && !p["at"]["keys"].is_array();
            json!({ "name": format!("p{k}"), "local": local, "at": (!follow).then_some(f.round(at)),
                "angle": angle, "follow": follow, "world": world, "bound": bound })
        })
        .collect()
}

fn tuned(mut spots: Vec<Value>, own: &Value, f: &Frame, t: f64) -> Vec<Value> {
    for (k, p) in own.as_array().into_iter().flatten().enumerate() {
        let Some(local) = pair(&value_at(p, t)) else { continue };
        if k >= spots.len() {
            spots.push(json!({ "name": format!("p{k}"), "angle": f.turn(), "follow": false, "world": false, "bound": false }));
        }
        let at = f.round(if spots[k]["world"] == true { local } else { f.apply(local) });
        (spots[k]["local"], spots[k]["at"], spots[k]["own"]) = (json!(local), json!(at), json!(true));
    }
    spots
}

pub(super) fn drawn_by<'a>(doc: &'a Value, system: &str) -> Option<&'a str> {
    let layers = crate::layers::walk(doc).into_iter().map(|(_, l)| l);
    layers.filter(|l| l["type"] == "particles" && l["system"] == system).find_map(|l| l["id"].as_str())
}

pub fn handles(dir: &Path, doc: &Value, system: &str, layer: Option<&str>, t: f64) -> Value {
    heard(dir, doc, system, layer, (t, 0.0))
}

pub fn heard(dir: &Path, doc: &Value, system: &str, layer: Option<&str>, (t, sound): (f64, f32)) -> Value {
    let Some(s) = doc.pointer(&format!("/scene/particles/{}", escape(system))) else { return Value::Null };
    let chosen = layer.and_then(|id| crate::actions::layer(doc, id).ok()).map(|(_, l)| l);
    let layer = layer.or_else(|| drawn_by(doc, system));
    let drawn = layer.and_then(|id| crate::actions::layer(doc, id).ok()).map(|(_, l)| l);
    let render = drawn.map(|l| &l["render"]).filter(|r| r.is_object()).unwrap_or(&s["render"]);
    let f = frame(dir, doc, layer, t);
    let spots = tuned(points(s, &f, t), chosen.map_or(&Value::Null, |l| &l["instance"]["points"]), &f, t);
    let children = s["children"].as_array().into_iter().flatten().enumerate().map(|(k, c)| {
        json!({ "index": k, "system": c["system"], "on": c["on"], "inherit": c.get("inherit").unwrap_or(&json!([])),
            "follow_inherit": c["follow_inherit"].as_bool().unwrap_or(false) })
    });
    let emitters = shapes(dir, doc, s, (&f, &spots), t);
    let (salt, seed) = super::salt::salt(doc, system);
    let count = spots.len();
    let inputs = [
        "daytime",
        "runtime",
        "life",
        "speed",
        "random",
        "cursor:x",
        "cursor:y",
        "audio:bass",
        "audio:mid",
        "audio:treble",
        "audio:level",
    ];
    let inputs: Vec<String> =
        inputs.map(String::from).into_iter().chain((0..count).map(|k| format!("distance:p{k}"))).collect();
    let mut steady = inputs.clone();
    steady.insert(3, "age".into());
    json!({
        "system": system,
        "layer": layer,
        "frame": f.m,
        "inverse": f.inverse(),
        "points": spots,
        "emitters": emitters,
        "perspective": s["perspective"],
        "depth": { "depth": s["init"]["depth"], "speed": s["init"]["depth_speed"] },
        "remaps": looks::remaps(s),
        "inputs": inputs,
        "steady_inputs": steady,
        "colliders": super::walls::colliders(dir, doc, s, &f, t),
        "keeps": super::steers::keeps(s, &f, &spots),
        "pulls": super::pulls::pulls(s, &f, &spots),
        "boids": super::steers::boids(s, &f),
        "operators": super::steers::operators(s),
        "strands": super::steers::strands(doc, s, render),
        "fields": super::view::fields(s, &f, &emitters, (t, salt, sound)),
        "sound": sound,
        "seed": seed,
        "outputs": ["size", "alpha", "color", "speed", "life", "rotation", "spin", "frame"],
        "domains": looks::domains(),
        "cutout": looks::cutout(render),
        "orient": { "mode": render.get("orient").unwrap_or(&json!("screen")), "angle": render["angle"].as_f64().unwrap_or(0.0) },
        "children": children.collect::<Vec<_>>(),
        "shaders": looks::shaders(doc, dir, render),
        "sources": looks::sources(doc, system),
    })
}
