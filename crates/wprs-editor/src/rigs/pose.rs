use super::round;
use serde_json::{Value, json};
use std::path::Path;
use wprs_scene::mesh_math::{IDENTITY, Mat, apply, conjugate, mul, qmul, turn};
use wprs_scene::{AnimClip, Key, Rigged, Track, Trs, Vec2, local, sample};

pub(crate) fn length(rig: &Rigged, c: &AnimClip) -> (Option<usize>, f32) {
    let motion = match &c.animation {
        Some(n) => rig.motions.iter().position(|m| &m.name == n),
        None if !c.tracked() && !rig.motions.is_empty() => Some(0),
        None => None,
    };
    let end = c.to.unwrap_or_else(|| motion.map_or(0.0, |m| rig.motions[m].length).max(c.end()));
    (motion, (end - c.from).max(0.0))
}

fn put<T: Clone>(keys: &mut Vec<Key<T>>, t: f32, v: Option<T>) {
    let Some(v) = v else { return };
    keys.retain(|k| (k.t - t).abs() >= 1e-3);
    let at = keys.partition_point(|k| k.t <= t);
    keys.insert(at, Key { t, v, ease: Default::default(), handles: None });
}

pub fn locals(rig: &Rigged, c: &AnimClip, t: f32) -> Vec<Trs> {
    let mut out: Vec<Trs> = rig.joints.iter().map(|j| j.rest).collect();
    let (motion, _) = length(rig, c);
    let t = t + c.from;
    let mut buffer = vec![0.0; 4];
    for ch in motion.map_or(&[][..], |m| &rig.motions[m].channels[..]) {
        let l = &mut out[ch.node as usize];
        buffer.resize(ch.width().max(4), 0.0);
        ch.sample(t, &mut buffer);
        match ch.path {
            Track::Translation => l.t = [buffer[0], buffer[1], buffer[2]],
            Track::Rotation => l.r = [buffer[0], buffer[1], buffer[2], buffer[3]],
            Track::Scale => l.s = [buffer[0], buffer[1], buffer[2]],
            Track::Weights => {}
        }
    }
    for (name, keys) in &c.bones {
        let Some(n) = rig.joint(name) else { continue };
        let rest = rig.joints[n].rest;
        let l = &mut out[n];
        if !keys.rotation.is_empty() {
            l.r = qmul(rest.r, turn(sample(&keys.rotation, t)));
        }
        if !keys.position.is_empty() {
            let [x, y] = sample(&keys.position, t).0;
            l.t = [rest.t[0] + x, rest.t[1] - y, rest.t[2]];
        }
        if !keys.scale.is_empty() {
            let [x, y] = sample(&keys.scale, t).0;
            l.s = [rest.s[0] * x, rest.s[1] * y, rest.s[2]];
        }
    }
    out
}

pub(crate) type Channels = (f32, [f32; 2], [f32; 2]);

pub(crate) fn channels(rig: &Rigged, c: &AnimClip, ls: &[Trs], n: usize, at: f32) -> Channels {
    let (rest, l) = (rig.joints[n].rest, ls[n]);
    let own = c.bones.get(&rig.joints[n].name);
    let q = qmul(conjugate(rest.r), l.r);
    let turned = (-(2.0 * q[2].atan2(q[3])).to_degrees() + 180.0).rem_euclid(360.0) - 180.0;
    let rotation = own.filter(|k| !k.rotation.is_empty()).map_or(turned, |k| sample(&k.rotation, at));
    let moved = [l.t[0] - rest.t[0], rest.t[1] - l.t[1]];
    let position = own.filter(|k| !k.position.is_empty()).map_or(moved, |k| sample(&k.position, at).0);
    let grown = [0, 1].map(|i| l.s[i] / if rest.s[i].abs() > 1e-6 { rest.s[i] } else { 1.0 });
    let scale = own.filter(|k| !k.scale.is_empty()).map_or(grown, |k| sample(&k.scale, at).0);
    (rotation, position, scale)
}

pub(crate) fn posed(rig: &Rigged, clip: &Value, bone: &str, t: f32) -> Option<Channels> {
    let c = serde_json::from_value::<AnimClip>(clip.clone()).ok()?;
    let n = rig.joint(bone)?;
    Some(channels(rig, &c, &locals(rig, &c, t), n, t + c.from))
}

pub fn worlds(rig: &Rigged, locals: &[Trs]) -> Vec<Mat> {
    let mut out = vec![IDENTITY; rig.joints.len()];
    for &n in &rig.order {
        let local = locals[n as usize].matrix();
        out[n as usize] = rig.joints[n as usize].parent.map_or(local, |p| mul(&out[p as usize], &local));
    }
    out
}

pub fn pose(dir: &Path, doc: &Value, id: &str, q: &Value) -> Value {
    let Ok((_, layer)) = crate::actions::layer(doc, id) else { return Value::Null };
    let Ok((_, _, rig)) = crate::skin::rigged(dir, doc, layer) else { return Value::Null };
    let Ok((_, clip)) = crate::clips::found(layer, q["clip"].as_str().unwrap_or_default()) else {
        return Value::Null;
    };
    let Ok(mut c) = serde_json::from_value::<AnimClip>(clip.clone()) else { return Value::Null };
    let (_, span) = length(&rig, &c);
    let t = local(q["time"].as_f64().unwrap_or(0.0) as f32, span, c.repeat);
    if let Some(b) = q["bone"].as_str().filter(|b| rig.joint(b).is_some()) {
        let keys = c.bones.entry(b.to_owned()).or_default();
        let pair = |v: &Value| Some(Vec2([v[0].as_f64()? as f32, v[1].as_f64()? as f32]));
        put(&mut keys.rotation, t + c.from, q["rotation"].as_f64().map(|v| v as f32));
        put(&mut keys.position, t + c.from, pair(&q["position"]));
        put(&mut keys.scale, t + c.from, pair(&q["scale"]));
    }
    let size = crate::skin::size(doc, layer);
    let ls = locals(&rig, &c, t);
    let ws = worlds(&rig, &ls);
    let picture =
        |p: [f32; 3]| [(p[0] + size[0] / 2.0) / size[0], (size[1] / 2.0 - p[1]) / size[1]].map(round);
    let mut nodes: Vec<usize> = Vec::new();
    for n in rig.palette.iter().map(|(n, _)| *n as usize) {
        if !nodes.contains(&n) {
            nodes.push(n);
        }
    }
    let bones: Vec<Value> = nodes
        .into_iter()
        .map(|n| {
            let w = &ws[n];
            let up = rig.joints[n].parent.map_or(IDENTITY, |p| ws[p as usize]);
            let frame = [up[0], up[1], up[4], up[5]].map(round);
            let (rotation, position, scale) = channels(&rig, &c, &ls, n, t + c.from);
            json!({
                "name": rig.joints[n].name,
                "at": picture(apply(w, [0.0; 3])),
                "angle": round((-w[4]).atan2(w[0]).to_degrees()),
                "frame": frame,
                "rotation": round(rotation),
                "position": position.map(round),
                "scale": scale.map(round),
            })
        })
        .collect();
    json!({ "time": round(t), "length": round(span), "size": size, "bones": bones })
}
