use crate::action::ClipEdit;
use crate::ops::{Op, set};
use anyhow::{Result, anyhow, ensure};
use serde_json::{Value, json};
use wprs_core::message::said;
use wprs_scene::MAX_CLIPS_PER_MODEL;

mod keyed;

pub(crate) use keyed::{key, num};

pub const MAX_LENGTH: f32 = 3600.0;

pub(crate) fn valid(id: &str) -> Result<()> {
    let fits = (1..=32).contains(&id.len())
        && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
    ensure!(fits, said("editor.clip_id", "a clip id has 1 to 32 small letters, digits or _", &[]));
    Ok(())
}

pub(crate) fn found<'a>(l: &'a Value, id: &str) -> Result<(usize, &'a Value)> {
    let clips = l["clips"].as_array().map_or(&[][..], Vec::as_slice);
    let i = clips.iter().position(|c| c["id"] == id);
    i.map(|i| (i, &clips[i]))
        .ok_or_else(|| anyhow!(said("editor.no_clip", "there is no clip \"%1\"", &[&id])))
}

fn field(doc: &Value, path: &str, key: &str, list: Vec<Value>) -> Vec<Op> {
    let at = format!("{path}/{key}");
    match (list.is_empty(), doc.pointer(&at).is_some()) {
        (true, true) => vec![Op::Remove { path: at }],
        (true, false) => Vec::new(),
        (false, _) => set(doc, &at, json!(list)),
    }
}

fn seed(dir: &std::path::Path, doc: &Value, l: &Value) -> Value {
    let Ok((_, _, rig)) = crate::skin::rigged(dir, doc, l) else { return json!({}) };
    let bone = rig
        .palette
        .iter()
        .map(|(n, _)| &rig.joints[*n as usize])
        .find(|j| j.parent.is_some() || rig.palette.len() > 1);
    match (rig.motions.is_empty(), bone, rig.parts.first()) {
        (false, _, _) => json!({}),
        (true, Some(b), _) => json!({ "bones": { b.name.clone(): { "rotation": [[0, 0]] } } }),
        (true, None, Some(p)) => json!({ "order": { p.clone(): [[0, 0]] } }),
        _ => json!({}),
    }
}

pub(crate) fn clip(dir: &std::path::Path, doc: &Value, c: &ClipEdit) -> Result<Vec<Op>> {
    let ClipEdit { layer, id, add, remove, to, length, repeat } = c;
    let (path, l) = crate::actions::layer(doc, layer)?;
    ensure!(l["type"] == "mesh", said("editor.not_mesh", "the layer is not a mesh", &[]));
    let mut clips = l["clips"].as_array().cloned().unwrap_or_default();
    let mut plays = l["animations"].as_array().cloned().unwrap_or_default();
    if let Some(n) = length {
        ensure!(
            n.is_finite() && *n > 0.0 && *n <= MAX_LENGTH,
            said("editor.clip_length", "a clip lasts more than 0 and at most %1 s", &[&MAX_LENGTH])
        );
    }
    let i = match (clips.iter().position(|c| c["id"] == id.as_str()), add) {
        (Some(_), true) => {
            return Err(anyhow!(said("editor.clip_taken", "there is already a clip \"%1\"", &[&id])));
        }
        (None, true) => {
            valid(id)?;
            ensure!(
                clips.len() < MAX_CLIPS_PER_MODEL,
                said("editor.clips_most", "a mesh has at most %1 clips", &[&MAX_CLIPS_PER_MODEL])
            );
            let mut new = json!({ "id": id, "to": length.unwrap_or(2.0) });
            new.as_object_mut()
                .into_iter()
                .for_each(|m| m.extend(seed(dir, doc, l).as_object().cloned().unwrap_or_default()));
            clips.push(new);
            clips.len() - 1
        }
        (None, false) => found(l, id).map(|f| f.0)?,
        (Some(i), false) => i,
    };
    if *remove {
        clips.remove(i);
        plays.retain(|a| a["clip"] != id.as_str());
    } else {
        let c = &mut clips[i];
        if let Some(n) = length {
            c["to"] = json!(c["from"].as_f64().unwrap_or(0.0) + f64::from(*n));
        }
        if let Some(r) = repeat {
            c["loop"] = json!(r);
        }
        if let Some(to) = to.as_deref().filter(|t| *t != id) {
            valid(to)?;
            let taken = l["clips"].as_array().into_iter().flatten().any(|c| c["id"] == to);
            ensure!(!taken, said("editor.clip_taken", "there is already a clip \"%1\"", &[&to]));
            c["id"] = json!(to);
            plays.iter_mut().filter(|a| a["clip"] == id.as_str()).for_each(|a| a["clip"] = json!(to));
        }
    }
    let mut ops = field(doc, &path, "clips", clips);
    if l.get("animations").is_some() {
        ops.extend(field(doc, &path, "animations", plays));
    }
    Ok(ops)
}
