use crate::action::ClipEdit;
use crate::ops::{Op, set};
use anyhow::{Result, anyhow, ensure};
use serde_json::{Value, json};
use wprs_core::message::said;
use wprs_scene::{MAX_ANIMATION_LAYERS, MAX_CLIPS_PER_MODEL};

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
    let ClipEdit { layer, id, add, remove, to, length, repeat, animation, from, play } = c;
    ensure!(
        !*play || *add,
        said("editor.clip_play_add", "only a clip being added starts playing in a new animation layer", &[])
    );
    let (path, l) = crate::actions::layer(doc, layer)?;
    let figure = l["type"] == "mesh" || l["type"] == "model";
    ensure!(figure, said("editor.not_figure", "the layer is not a mesh or a model", &[]));
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
        if let Some(name) = animation {
            let (_, _, rig) = crate::skin::rigged(dir, doc, l)?;
            let m = rig.motion(Some(name)).ok_or_else(|| {
                anyhow!(said("editor.no_animation", "the file has no animation \"%1\"", &[name]))
            })?;
            c["animation"] = json!(name);
            c["to"] = json!(f64::from(m.length));
            drop(c.as_object_mut().and_then(|o| o.remove("from")));
        }
        if let Some(f) = from {
            ensure!(
                f.is_finite() && (0.0..=MAX_LENGTH).contains(f),
                said("editor.time_must_more", "the time must be 0 or more seconds", &[])
            );
            let span = c["to"].as_f64().unwrap_or(2.0) - c["from"].as_f64().unwrap_or(0.0);
            let end = match c["animation"].as_str() {
                Some(a) => crate::skin::rigged(dir, doc, l)?
                    .2
                    .motion(Some(a))
                    .map_or(f64::MAX, |m| f64::from(m.length)),
                None => f64::MAX,
            };
            ensure!(
                f64::from(*f) < end,
                said("editor.time_must_more", "the time must be 0 or more seconds", &[])
            );
            c["from"] = json!(f);
            c["to"] = json!((f64::from(*f) + span.max(0.0)).min(end));
        }
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
    if *play {
        ensure!(
            plays.len() < MAX_ANIMATION_LAYERS,
            said("editor.plays_most", "a layer has at most %1 animation layers", &[&MAX_ANIMATION_LAYERS])
        );
        plays.push(json!({ "clip": id }));
    }
    let mut ops = field(doc, &path, "clips", clips);
    if l.get("animations").is_some() || *play {
        ops.extend(field(doc, &path, "animations", plays));
    }
    Ok(ops)
}
