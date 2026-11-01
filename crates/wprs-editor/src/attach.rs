use crate::action::{AttachEdit, Spot};
use crate::clips::num;
use crate::limbs::{joint, joints};
use crate::ops::{Op, set};
use crate::skin::Skin;
use anyhow::{Result, anyhow, bail, ensure};
use serde_json::{Value, json};
use std::path::Path;
use wprs_core::message::said;

mod nudge;

fn follows(doc: &Value, from: &str, own: &str) -> bool {
    let mut at = from.to_owned();
    for _ in 0..64 {
        let Ok((_, l)) = crate::actions::layer(doc, &at) else { return false };
        match l["attach"]["layer"].as_str() {
            Some(next) if next == own => return true,
            Some(next) => at = next.to_owned(),
            None => return false,
        }
    }
    true
}

fn orphan(doc: &Value, own: &str, old: &Value) -> Option<(String, String)> {
    let (mesh, name) = (old["layer"].as_str()?, old["point"].as_str()?);
    let shared = crate::layers::walk(doc)
        .iter()
        .any(|(_, l)| l["id"] != own && l["attach"]["layer"] == mesh && l["attach"]["point"] == name);
    let (_, m) = crate::actions::layer(doc, mesh).ok()?;
    let aim = json!(format!("bone:{name}"));
    let aimed = m["ik"].as_array().into_iter().flatten().any(|c| c["target"] == aim);
    (!shared && !aimed && m["type"] == "mesh").then(|| (mesh.to_owned(), name.to_owned()))
}

fn point(
    dir: &Path,
    doc: &Value,
    mesh: &Value,
    a: &AttachEdit,
    at: [f32; 2],
    reuse: Option<&str>,
) -> Result<(Vec<Op>, String)> {
    ensure!(
        at.iter().all(|v| v.is_finite()),
        said("editor.point_at", "a point is placed in fractions of the layer", &[])
    );
    let (id, mut skin) = Skin::load(dir, doc, mesh)?;
    let at = skin.model(at.map(|v| v.clamp(-1.0, 2.0)));
    let name = match reuse.and_then(|n| skin.point(n).ok()) {
        Some(k) => {
            let lead = skin.lead(at);
            skin.hang(k, lead)?;
            skin.place(k, at);
            skin.names[k].clone()
        }
        None => {
            let base = format!("{}_point", a.layer);
            let free = |n: &String| !skin.names.contains(n);
            let mut names = std::iter::once(base.clone()).chain((2..).map(|i| format!("{base}_{i}")));
            let name = names.find(free).unwrap_or(base);
            skin.add_point(&name, at)?;
            name
        }
    };
    Ok((skin.save(dir, doc, &id)?, name))
}

fn unpin(dir: &Path, doc: &Value, mesh: &str, name: &str) -> Result<Vec<Op>> {
    let (_, layer) = crate::actions::layer(doc, mesh)?;
    let (id, mut skin) = Skin::load(dir, doc, layer)?;
    let Ok(k) = skin.point(name) else { return Ok(Vec::new()) };
    skin.remove_bone(k)?;
    skin.save(dir, doc, &id)
}

pub(crate) fn attach(dir: &Path, doc: &Value, a: &AttachEdit) -> Result<Vec<Op>> {
    let (path, layer) = crate::actions::layer(doc, &a.layer)?;
    let at = format!("{path}/attach");
    let old = layer.get("attach").cloned().unwrap_or(Value::Null);
    let spare = orphan(doc, &a.layer, &old);
    if a.detach {
        ensure!(!old.is_null(), said("editor.not_attached", "the layer follows no bone", &[]));
        let mut ops = match &spare {
            Some((mesh, name)) => unpin(dir, doc, mesh, name)?,
            None => Vec::new(),
        };
        ops.push(Op::Remove { path: at });
        return Ok(ops);
    }
    let to = a.to.clone().or_else(|| old["layer"].as_str().map(String::from));
    let to = to.ok_or_else(|| anyhow!(said("editor.attach_to", "say which mesh layer it follows", &[])))?;
    ensure!(
        to != a.layer && !follows(doc, &to, &a.layer),
        said("editor.attach_self", "a layer cannot follow itself, through other layers either", &[])
    );
    let (_, mesh) = crate::actions::layer(doc, &to)?;
    ensure!(
        ["mesh", "model"].contains(&mesh["type"].as_str().unwrap_or_default()),
        said("editor.attach_mesh", "a layer follows a bone or point of a mesh or model layer", &[])
    );
    let same = old["layer"] == to.as_str();
    let named = |n: &str| match mesh["type"] == "mesh" {
        true => joints(dir, doc, mesh).and_then(|j| joint(&j, n)).map(drop),
        false => Ok(()),
    };
    let mut ops = Vec::new();
    let mut out = json!({ "layer": to });
    match (&a.bone, &a.point) {
        (Some(_), Some(_)) => {
            bail!(said("editor.attach_one", "a layer follows a bone or a point, not both", &[]))
        }
        (Some(b), None) => {
            named(b)?;
            out["bone"] = json!(b);
        }
        (None, Some(Spot::Name(n))) => {
            named(n)?;
            out["point"] = json!(n);
        }
        (None, Some(Spot::At(p))) => {
            ensure!(
                mesh["type"] == "mesh",
                said("editor.attach_place", "points are placed on mesh layers", &[])
            );
            let reuse = spare.as_ref().filter(|(m, _)| *m == to).map(|(_, n)| n.as_str());
            let (saved, name) = point(dir, doc, mesh, a, *p, reuse)?;
            ops = saved;
            out["point"] = json!(name);
        }
        (None, None) if same && (old.get("bone").is_some() || old.get("point").is_some()) => {
            ["bone", "point"]
                .iter()
                .filter_map(|k| Some((k, old.get(*k)?)))
                .for_each(|(k, v)| out[*k] = v.clone());
        }
        (None, None) => bail!(said("editor.attach_which", "say which bone or point it follows", &[])),
    }
    if let Some((mesh, name)) =
        spare.filter(|(m, n)| out["layer"] != m.as_str() || out["point"] != n.as_str())
    {
        ops.extend(unpin(dir, doc, &mesh, &name)?);
    }
    match a.offset {
        Some(o) if o.iter().all(|v| v.is_finite()) => out["offset"] = json!(o.map(num)),
        Some(_) => bail!(said("editor.attach_offset", "an offset is two numbers of pixels", &[])),
        None if same && old.get("offset").is_some() => out["offset"] = old["offset"].clone(),
        None => {}
    }
    match &a.inherit {
        Some(list) if list.len() < 3 => out["inherit"] = json!(list),
        Some(_) => {}
        None if same && old.get("inherit").is_some() => out["inherit"] = old["inherit"].clone(),
        None => {}
    }
    if let Some(by) = a.nudge {
        nudge::by(dir, doc, mesh, &mut out, by)?;
    }
    ops.extend(set(doc, &at, out));
    Ok(ops)
}
