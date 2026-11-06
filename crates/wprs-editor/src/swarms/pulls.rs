use super::shapes::{Frame, target};
use serde_json::{Value, json};

const KINDS: [(&str, &str, &[&str]); 4] = [
    ("attractor", "target", &["radius"]),
    ("vortex", "center", &["radius", "inner", "outer"]),
    ("orbit", "center", &["radius"]),
    ("slow_near", "target", &["inner", "outer"]),
];

pub(super) fn pulls(s: &Value, f: &Frame, spots: &[Value]) -> Vec<Value> {
    let list = s["forces"].as_array().into_iter().flatten().enumerate();
    list.filter_map(|(k, o)| {
        let (kind, field, reach) = KINDS.iter().find(|(kind, ..)| o.get(*kind).is_some())?;
        let v = &o[*kind];
        let (at, _) = target(&v[*field], f, spots);
        let mut rings: Vec<(&str, f64)> =
            reach.iter().filter_map(|key| Some((*key, v[*key].as_f64()?))).collect();
        if let Some(r) = v["ring"]["radius"].as_f64() {
            rings.push(("ring", r));
        }
        let rings: Vec<Value> = rings
            .into_iter()
            .map(|(key, value)| {
                let px = value * f.scale();
                json!({ "key": key, "value": value, "px": px, "at": at.map(|c| f.round([c[0], c[1] - px])) })
            })
            .collect();
        Some(
            json!({ "index": k, "kind": kind, "field": field, "ref": v[*field], "at": at.map(|c| f.round(c)),
            "strength": v["strength"], "factor": v["factor"], "rings": rings }),
        )
    })
    .collect()
}
