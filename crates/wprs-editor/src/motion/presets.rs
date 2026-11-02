use super::{field, figure};
use crate::clips::num;
use crate::ops::Op;
use anyhow::{Result, anyhow, ensure};
use serde_json::{Value, json};
use std::path::Path;
use wprs_core::message::said;
use wprs_scene::MAX_BODIES;

pub const PRESETS: [(&str, &str, [f32; 5]); 6] = [
    ("hair", "rope", [0.2, 0.3, 0.7, 1.0, 0.6]),
    ("tail", "rope", [0.4, 0.4, 0.5, 0.5, 0.3]),
    ("cloth", "spring", [0.3, 0.5, 0.6, 2.0, 0.8]),
    ("antenna", "spring", [0.7, 0.2, 0.4, 0.0, 0.2]),
    ("chain", "rigid", [0.1, 0.2, 0.8, 3.0, 0.1]),
    ("jelly", "spring", [0.5, 0.1, 0.9, 0.0, 0.0]),
];

pub(super) fn preset(dir: &Path, doc: &Value, (layer, bone): (&str, &str), name: &str) -> Result<Vec<Op>> {
    let (path, l) = figure(doc, layer)?;
    let (_, _, rig) = crate::skin::rigged(dir, doc, l)?;
    ensure!(rig.joint(bone).is_some(), said("editor.no_bone", "there is no bone \"%1\"", &[&bone]));
    let (_, mode, [stiffness, damping, inertia, gravity, wind]) =
        PRESETS.iter().find(|(n, ..)| *n == name).copied().ok_or_else(|| {
            let names = PRESETS.map(|p| p.0).join(", ");
            anyhow!(said("editor.no_preset", "the presets are %1", &[&names]))
        })?;
    let mut bodies = l["physics"].as_array().cloned().unwrap_or_default();
    let at = match bodies.iter().position(|b| b["bone"] == bone) {
        Some(i) => i,
        None => {
            ensure!(
                bodies.len() < MAX_BODIES,
                said("editor.bodies_most", "a layer has at most %1 swinging bones", &[&MAX_BODIES])
            );
            bodies.push(json!({ "bone": bone }));
            bodies.len() - 1
        }
    };
    let b = &mut bodies[at];
    b["mode"] = json!(mode);
    let values = [
        ("stiffness", stiffness),
        ("damping", damping),
        ("inertia", inertia),
        ("gravity", gravity),
        ("wind", wind),
    ];
    values.into_iter().for_each(|(k, v)| b[k] = num(v));
    Ok(field(doc, &path, "physics", Some(json!(bodies))))
}
