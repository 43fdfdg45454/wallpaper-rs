use crate::action::{IkEdit, WindEdit};
use crate::clips::num;
use crate::ops::{Op, set};
use anyhow::{Result, anyhow, bail, ensure};
use serde_json::{Value, json};
use std::path::Path;
use wprs_core::message::said;
use wprs_scene::{MAX_CHAIN, MAX_CHAINS};

pub(crate) type Joints = Vec<(String, Option<usize>)>;

pub(crate) fn joints(dir: &Path, doc: &Value, layer: &Value) -> Result<Joints> {
    let (_, _, rig) = crate::skin::rigged(dir, doc, layer)?;
    Ok(rig.joints.iter().map(|j| (j.name.clone(), j.parent.map(usize::from))).collect())
}

pub(crate) fn joint(j: &Joints, name: &str) -> Result<usize> {
    let found = j.iter().position(|(n, _)| n == name);
    found.ok_or_else(|| anyhow!(said("editor.no_bone", "there is no bone \"%1\"", &[&name])))
}

pub(crate) fn listed(doc: &Value, path: &str, list: Vec<Value>) -> Vec<Op> {
    match (list.is_empty(), doc.pointer(path).is_some()) {
        (true, true) => vec![Op::Remove { path: path.into() }],
        (true, false) => Vec::new(),
        (false, _) => set(doc, path, json!(list)),
    }
}

fn chain(j: &Joints, root: &str, tip: &str) -> Result<Vec<String>> {
    let (top, mut at) = (joint(j, root)?, joint(j, tip)?);
    let mut out = vec![j[at].0.clone()];
    while at != top {
        let up = j[at].1.filter(|_| out.len() < MAX_CHAIN);
        at = up.ok_or_else(|| {
            anyhow!(said(
                "editor.chain_hangs",
                "a chain goes from a bone down to one that hangs below it, 2 to %1 bones",
                &[&MAX_CHAIN]
            ))
        })?;
        out.insert(0, j[at].0.clone());
    }
    ensure!(
        out.len() >= 2,
        said(
            "editor.chain_hangs",
            "a chain goes from a bone down to one that hangs below it, 2 to %1 bones",
            &[&MAX_CHAIN]
        )
    );
    Ok(out)
}

fn target(doc: &Value, j: &Joints, own: &str, t: &Value) -> Result<Value> {
    let bad = || {
        anyhow!(said(
            "editor.chain_target",
            "a chain reaches for cursor, bone:<name>, layer:<id> or a point [x, y]",
            &[]
        ))
    };
    match t {
        Value::String(s) if s == "cursor" => {}
        Value::String(s) if s.starts_with("bone:") => drop(joint(j, &s[5..])?),
        Value::String(s) if s.starts_with("layer:") && &s[6..] != own => {
            drop(crate::actions::layer(doc, &s[6..])?)
        }
        Value::Array(p) if p.len() == 2 && p.iter().all(|v| v.as_f64().is_some_and(f64::is_finite)) => {}
        _ => return Err(bad()),
    }
    Ok(t.clone())
}

fn edit(c: &mut Value, k: &IkEdit, j: &Joints, doc: &Value) -> Result<()> {
    if let (Some(root), Some(tip)) = (&k.root, &k.tip) {
        c["chain"] = json!(chain(j, root, tip)?);
    }
    if let Some(t) = &k.target {
        c["target"] = target(doc, j, &k.layer, t)?;
    }
    let mut limits = c["limits"].as_object().cloned().unwrap_or_default();
    for (bone, [lo, hi]) in &k.limits {
        ensure!(
            c["chain"].as_array().is_some_and(|l| l.contains(&json!(bone))),
            said("editor.limit_chain", "a limit names a bone of its chain", &[])
        );
        let fits = lo.is_finite() && hi.is_finite() && -180.0 <= *lo && lo <= hi && *hi <= 180.0;
        ensure!(
            fits,
            said("editor.limit_range", "a limit goes from a smaller to a larger angle, -180 to 180", &[])
        );
        limits.insert(bone.clone(), json!([num(*lo), num(*hi)]));
    }
    k.unlimit.iter().for_each(|b| drop(limits.remove(b)));
    limits.retain(|b, _| c["chain"].as_array().is_some_and(|l| l.contains(&json!(b))));
    let map = c.as_object_mut().ok_or_else(|| anyhow!("the chain is not an object"))?;
    match limits.is_empty() {
        true => drop(map.remove("limits")),
        false => drop(map.insert("limits".into(), Value::Object(limits))),
    }
    if let Some(w) = k.weight {
        ensure!(
            w.is_finite() && (0.0..=1.0).contains(&w),
            said("editor.chain_weight", "a chain weighs from 0 to 1", &[])
        );
        map.insert("weight".into(), num(w));
    }
    if let Some(b) = k.bend {
        map.insert("bend".into(), json!(b));
    }
    Ok(())
}

pub(crate) fn ik(dir: &Path, doc: &Value, k: &IkEdit) -> Result<Vec<Op>> {
    let (path, layer) = crate::actions::layer(doc, &k.layer)?;
    let j = joints(dir, doc, layer)?;
    let mut list = layer["ik"].as_array().cloned().unwrap_or_default();
    let n = list.len();
    let no = |i: usize| anyhow!(said("editor.no_chain", "there is no chain %1", &[&i]));
    match k.index {
        Some(i) if i >= n => return Err(no(i)),
        Some(i) if k.remove => drop(list.remove(i)),
        Some(i) => edit(&mut list[i], k, &j, doc)?,
        None if k.remove => bail!(said("editor.chain_which", "say which chain changes", &[])),
        None => {
            ensure!(
                n < MAX_CHAINS,
                said("editor.chains_most", "a mesh has at most %1 chains", &[&MAX_CHAINS])
            );
            let (Some(root), Some(tip)) = (&k.root, &k.tip) else {
                bail!(said("editor.chain_ends", "a new chain names its root and tip bones", &[]))
            };
            let rest = crate::rigs::rest_end(dir, doc, &k.layer, tip);
            let point = json!(rest.map_or([0.0; 2], |p| p.map(|v| (f64::from(v) * 100.0).round() / 100.0)));
            let mut c = json!({ "chain": chain(&j, root, tip)?, "target": point });
            edit(&mut c, k, &j, doc)?;
            list.push(c);
        }
    }
    Ok(listed(doc, &format!("{path}/ik"), list))
}

pub(crate) fn wind(doc: &Value, w: &WindEdit) -> Result<Vec<Op>> {
    let canvas = &doc["scene"]["canvas"];
    if w.off {
        let gone = ["wind", "gusts"].into_iter().filter(|k| canvas.get(*k).is_some());
        return Ok(gone.map(|k| Op::Remove { path: format!("/scene/canvas/{k}") }).collect());
    }
    ensure!(
        w.angle.is_some() || w.strength.is_some() || w.gusts.is_some(),
        said("editor.wind_what", "say the angle, strength or gusts of the wind", &[])
    );
    let now = crate::rigs::wind_arrow(doc);
    let mut ops = Vec::new();
    if w.angle.is_some() || w.strength.is_some() {
        let angle = w.angle.map_or(now["angle"].as_f64().unwrap_or(0.0), f64::from);
        let strength = w.strength.map_or(now["strength"].as_f64().unwrap_or(0.0), f64::from);
        let fits = angle.is_finite() && strength.is_finite() && (0.0..=10_000.0).contains(&strength);
        ensure!(fits, said("editor.wind_strength", "the wind blows from 0 to 10000 pixels a second", &[]));
        let (s, c) = angle.to_radians().sin_cos();
        let at = |v: f64| (v * strength * 100.0).round() / 100.0 + 0.0;
        ops.extend(set(doc, "/scene/canvas/wind", json!([at(c), at(s)])));
    }
    if let Some(g) = w.gusts {
        ensure!(
            g.is_finite() && (0.0..=1.0).contains(&g),
            said("editor.wind_gusts", "gusts go from 0 to 1", &[])
        );
        ops.extend(set(doc, "/scene/canvas/gusts", num(g)));
    }
    Ok(ops)
}
