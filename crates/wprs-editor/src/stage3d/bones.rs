use super::gizmo::{Seen, px, world};
use serde_json::{Value, json};
use wprs_scene::{Placed, Runtime, mul4, placement};

pub(super) fn bones(rt: &Runtime, seen: &Seen, p: &Placed, mut out: Value) -> Value {
    let Some(rig) = p.model.rig.as_ref() else { return out };
    let Some(puppet) = rt.puppet(&p.layer.id) else { return out };
    let worlds = puppet.joint_worlds();
    let head = |n: usize| worlds.get(n).map(|w| world(&p.matrix, [w[3], w[7], w[11]]));
    let mut list = Vec::new();
    for (n, j) in rig.joints.iter().enumerate().filter(|(_, j)| !j.point) {
        let Some(at) = head(n).and_then(|h| seen.at(h)) else { continue };
        let up = j.parent.and_then(|q| head(q as usize)).and_then(|h| seen.at(h));
        list.push(json!({ "name": j.name, "at": px(at), "from": up.map(px) }));
    }
    out["bones"] = json!(list);
    let layer = &p.layer;
    let root = matches!(&layer.kind, wprs_scene::Kind::Model(m) if m.root_motion);
    let clip = match &layer.kind {
        wprs_scene::Kind::Model(m) => {
            m.rig.animations.first().and_then(|a| m.rig.clips.iter().find(|c| c.id == a.clip))
        }
        _ => None,
    };
    if let Some(c) = clip.filter(|_| root) {
        let travel = puppet.posed().travel;
        let home = mul4(&p.matrix, &placement(travel.map(|v| -v), [0.0; 3], [1.0; 3]));
        let clip = serde_json::to_value(c).unwrap_or_default();
        let path = super::super::motion::root_path(rig, &clip, 2.0);
        out["path"] =
            json!(path.iter().filter_map(|q| seen.at(world(&home, *q)).map(px)).collect::<Vec<_>>());
    }
    out
}
