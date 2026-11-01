use crate::action::BodyEdit;
use crate::clips::num;
use crate::limbs::{joint, joints, listed};
use crate::ops::Op;
use crate::skin::Skin;
use anyhow::{Result, ensure};
use serde_json::{Map, Value, json};
use std::path::Path;
use wprs_core::message::said;
use wprs_scene::{Body, MAX_BODIES, MAX_CHAIN, MAX_SKETCH_BONES};

fn tune(entry: &mut Map<String, Value>, b: &BodyEdit) -> Result<()> {
    let unit = [("stiffness", b.stiffness), ("damping", b.damping), ("inertia", b.inertia), ("wind", b.wind)];
    for (key, v) in unit.into_iter().filter_map(|(k, v)| Some((k, v?))) {
        ensure!(
            v.is_finite() && (0.0..=1.0).contains(&v),
            said("editor.body_unit", "%1 goes from 0 to 1", &[&key])
        );
        entry.insert(key.into(), num(v));
    }
    if let Some(g) = b.gravity {
        ensure!(
            g.is_finite() && (-10.0..=10.0).contains(&g),
            said("editor.body_gravity", "gravity goes from -10 to 10", &[])
        );
        entry.insert("gravity".into(), num(g));
    }
    for (key, v, most) in [("rotation", b.rotation, 180.0), ("translation", b.translation, 1e4)] {
        let Some([lo, hi]) = v else { continue };
        let fits = lo.is_finite() && hi.is_finite() && -most <= lo && lo <= hi && hi <= most;
        ensure!(
            fits,
            said("editor.body_bounds", "%1 goes from a smaller to a larger value, within %2", &[&key, &most])
        );
        entry.insert(key.into(), json!([num(lo), num(hi)]));
    }
    if let Some(m) = b.mode {
        entry.insert("mode".into(), json!(m));
    }
    Ok(())
}

fn rope(dir: &Path, doc: &Value, layer: &Value, b: &BodyEdit) -> Result<(Vec<Op>, String)> {
    let [base] = b.bones.as_slice() else {
        anyhow::bail!(said("editor.rope_name", "a rope takes one name for its bones", &[]))
    };
    ensure!(
        (2..=MAX_CHAIN + 1).contains(&b.rope.len()),
        said("editor.rope_points", "a rope has 2 to %1 points", &[&(MAX_CHAIN + 1)])
    );
    let (id, mut skin) = Skin::load(dir, doc, layer)?;
    ensure!(
        skin.bones.len() + b.rope.len() <= MAX_SKETCH_BONES,
        said("editor.bones_most", "a mesh has at most %1 bones here", &[&MAX_SKETCH_BONES])
    );
    let mut up = b.parent.as_deref().map(|p| skin.bone(p)).transpose()?;
    for (i, at) in b.rope.iter().enumerate() {
        let at = skin.model(at.map(|v| if v.is_finite() { v.clamp(-1.0, 2.0) } else { 0.5 }));
        up = Some(skin.add_bone(&format!("{base}_{}", i + 1), up, at)?);
    }
    Ok((skin.save(dir, doc, &id)?, format!("{base}_1")))
}

pub(crate) fn physics(dir: &Path, doc: &Value, b: &BodyEdit) -> Result<Vec<Op>> {
    let (path, layer) = crate::actions::layer(doc, &b.layer)?;
    ensure!(!b.bones.is_empty(), said("editor.body_bones", "name the bones that swing", &[]));
    let (mut ops, bones) = match b.rope.is_empty() {
        true => {
            let j = joints(dir, doc, layer)?;
            b.bones.iter().try_for_each(|n| joint(&j, n).map(drop))?;
            (Vec::new(), b.bones.clone())
        }
        false if b.remove => {
            anyhow::bail!(said("editor.rope_remove", "a rope is removed by its first bone", &[]))
        }
        false => {
            let (ops, first) = rope(dir, doc, layer, b)?;
            (ops, vec![first])
        }
    };
    let mut list = layer["physics"].as_array().cloned().unwrap_or_default();
    let has = |list: &[Value], n: &str| list.iter().position(|e| e["bone"] == n);
    for n in &bones {
        match (has(&list, n), b.remove) {
            (Some(i), true) => drop(list.remove(i)),
            (None, true) => anyhow::bail!(said("editor.no_body", "the bone \"%1\" does not swing", &[n])),
            (found, false) => {
                let i = found.unwrap_or_else(|| {
                    let mode = if b.rope.is_empty() { Body::Spring } else { Body::Rope };
                    list.push(json!({ "bone": n, "mode": mode }));
                    list.len() - 1
                });
                if let Some(entry) = list[i].as_object_mut() {
                    tune(entry, b)?;
                }
            }
        }
    }
    ensure!(
        list.len() <= MAX_BODIES,
        said("editor.bodies_most", "a mesh has at most %1 swinging bones", &[&MAX_BODIES])
    );
    ops.extend(listed(doc, &format!("{path}/physics"), list));
    Ok(ops)
}
