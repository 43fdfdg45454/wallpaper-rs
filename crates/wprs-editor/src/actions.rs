use crate::arrange;
use crate::keys::{Key, keyframe};
use crate::layers::{children, clean, find, parent, system, taken, template, unique, walk};
use crate::ops::{Op, apply, set};
use anyhow::{Result, anyhow, ensure};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use wprs_core::message::said;
use wprs_format::features;

pub use crate::action::{ACTIONS, Action, EDITOR, KeyEase};
pub use crate::action::{
    AttachEdit, AutoWeights, BodyEdit, BoneEdit, ClipEdit, ClipKey, DepthStroke, ExpressionEdit, IkEdit,
    MeshCut, PartsEdit, PointEdit, Spot, WeightStroke, WindEdit,
};

pub fn layer<'a>(doc: &'a Value, id: &str) -> Result<(String, &'a Value)> {
    let path = find(doc, id)?;
    Ok((
        path.clone(),
        doc.pointer(&path)
            .ok_or_else(|| anyhow!(said("editor.there_no_layer", "there is no layer \"%1\"", &[&id])))?,
    ))
}

fn listed(doc: &Value, key: &str) -> Vec<String> {
    let list = doc.pointer(&format!("{EDITOR}/{key}")).and_then(Value::as_array).into_iter().flatten();
    list.filter_map(|v| v.as_str().map(String::from)).collect()
}

pub fn locked(doc: &Value) -> Vec<String> {
    listed(doc, "locked")
}

pub fn solo(doc: &Value) -> Option<&str> {
    doc.pointer(&format!("{EDITOR}/solo")).and_then(Value::as_str)
}

fn computed(doc: &Value, ops: &[Op]) -> Result<()> {
    const AT: &str = "/manifest/requires/features";
    if !ops.iter().flat_map(Op::paths).any(|p| p.starts_with(AT)) {
        return Ok(());
    }
    let names = |d: &Value| -> Option<BTreeSet<String>> {
        match d.pointer(AT) {
            None => Some(BTreeSet::new()),
            Some(list) => list.as_array()?.iter().map(|v| v.as_str().map(String::from)).collect(),
        }
    };
    let mut probe = doc.clone();
    let chosen = apply(&mut probe, ops).ok().and_then(|_| Some((names(doc)?, names(&probe)?)));
    let only = |(old, new): (BTreeSet<String>, BTreeSet<String>)| {
        old.symmetric_difference(&new).all(|f| features::CHOSEN.contains(&f.as_str()))
    };
    ensure!(
        chosen.is_some_and(only),
        said(
            "editor.requires_features_computed",
            "requires.features is computed from what the package uses, except locale_files",
            &[]
        )
    );
    Ok(())
}

pub fn plan(doc: &Value, action: &Action) -> Result<Vec<Op>> {
    let ops = steps(doc, action)?;
    if let Action::Set { .. }
    | Action::Add { .. }
    | Action::Remove { .. }
    | Action::Translate { .. }
    | Action::Move { .. } = action
    {
        computed(doc, &ops)?;
    }
    Ok(ops)
}

fn steps(doc: &Value, action: &Action) -> Result<Vec<Op>> {
    Ok(match action {
        Action::Set { pointer, value } => set(doc, pointer, value.clone()),
        Action::Add { pointer, value } => vec![Op::Add { path: pointer.clone(), value: value.clone() }],
        Action::Remove { pointer } => vec![Op::Remove { path: pointer.clone() }],
        Action::Move { from, to } => vec![Op::Move { from: from.clone(), path: to.clone() }],
        Action::AddLayer { kind, id, name, source, into, at } => {
            if let Some(id) = id {
                ensure!(!taken(doc, id), said("editor.id_use", "the id %1 is in use", &[&id]));
            }
            let base = name.as_deref().map(clean).filter(|c| !c.is_empty()).unwrap_or_else(|| kind.clone());
            let id = id.clone().unwrap_or_else(|| unique(doc, &base));
            let list = children(doc, into.as_deref())?;
            let len = doc.pointer(&list).and_then(Value::as_array).map_or(0, Vec::len);
            let mut ops = Vec::new();
            let named = source.clone().unwrap_or_else(|| id.clone());
            if kind == "particles" && doc.pointer(&format!("/scene/particles/{named}")).is_none() {
                ops.extend(set(doc, &format!("/scene/particles/{named}"), system()));
            }
            let path = format!("{list}/{}", at.unwrap_or(len).min(len));
            ops.push(Op::Add { path, value: template(doc, kind, &id, source.as_deref(), name.as_deref())? });
            ops
        }
        Action::RemoveLayer { id } => {
            let (path, value) = layer(doc, id)?;
            let mut ops = vec![Op::Remove { path }];
            ops.extend(arrange::forget(doc, value));
            if solo(doc) == Some(id) {
                ops.push(Op::Remove { path: format!("{EDITOR}/solo") });
            }
            let mut list = locked(doc);
            if list.contains(id) {
                list.retain(|l| l != id);
                ops.extend(set(doc, &format!("{EDITOR}/locked"), json!(list)));
            }
            ops
        }
        Action::DuplicateLayer { id } => arrange::duplicate(doc, id)?,
        Action::MoveLayer { id, to, into } => {
            let from = find(doc, id)?;
            let list = children(doc, into.as_deref())?;
            let len = doc.pointer(&list).and_then(Value::as_array).map_or(0, Vec::len);
            let len = if parent(&from).0 == list { len - 1 } else { len };
            vec![Op::Move { from, path: format!("{list}/{}", (*to).min(len)) }]
        }
        Action::Group { ids, id } => arrange::group(doc, ids, id.as_deref())?,
        Action::Ungroup { id } => arrange::ungroup(doc, id)?,
        Action::Hide { id, hidden } => arrange::hide(doc, id, *hidden)?,
        Action::Lock { id, locked } => {
            find(doc, id)?;
            let mut list = listed(doc, "locked");
            list.retain(|l| l != id);
            if *locked {
                list.push(id.clone());
            }
            set(doc, &format!("{EDITOR}/locked"), json!(list))
        }
        Action::Solo { id: Some(id) } => {
            find(doc, id)?;
            set(doc, &format!("{EDITOR}/solo"), json!(id))
        }
        Action::Solo { id: None } => match solo(doc) {
            Some(_) => vec![Op::Remove { path: format!("{EDITOR}/solo") }],
            None => Vec::new(),
        },
        Action::Keyframe { pointer, time, value, ease, timeline, remove, from } => {
            let key = Key {
                time: *time,
                value: value.clone(),
                ease: ease.clone(),
                timeline: timeline.as_deref(),
                remove: *remove,
                from: *from,
            };
            keyframe(doc, pointer, key)?
        }
        Action::Translate { pointer, language, text, add } => {
            crate::texts::translate(doc, pointer, language, text.as_deref(), *add)?
        }
        Action::Upgrade { rating, reasons, photosensitive, derived_from } => {
            crate::meta::upgrade(doc, *rating, reasons, *photosensitive, derived_from)?
        }
        Action::AddProperty { kind, id, group, label } => {
            crate::props::add_property(doc, kind, id.as_deref(), group.as_deref(), label.as_deref())?
        }
        Action::MoveProperty { id, to, group } => {
            crate::props::move_property(doc, id, *to, group.as_deref())?
        }
        Action::RenameProperty { id, to } => crate::references::rename_property(doc, id, to)?,
        Action::RemoveProperty { id } => crate::references::remove_property(doc, id)?,
        Action::Condition { id, enabled, text } => crate::references::condition(doc, id, *enabled, text)?,
        Action::AddPreset { id, label } => crate::props::add_preset(doc, id.as_deref(), label.as_deref())?,
        Action::Import { .. } | Action::ImportEffect { .. } | Action::ImportScript { .. } => Vec::new(),
        Action::Thumbnail { .. } | Action::PaintMask { .. } => Vec::new(),
        Action::MeshFromPicture(_) | Action::Bone(_) | Action::MeshDepth(_) | Action::Weights(_) => {
            Vec::new()
        }
        Action::AutoWeights(_) | Action::Clip(_) | Action::Key(_) | Action::Expression(_) => Vec::new(),
        Action::Ik(_) | Action::Physics(_) | Action::Wind(_) | Action::Attach(_) | Action::Parts(_) => {
            Vec::new()
        }
        Action::Point(_) | Action::Undo {} | Action::Redo {} => Vec::new(),
    })
}

pub fn visible(doc: &Value) -> Value {
    let mut scene = doc["scene"].clone();
    let Some(id) = solo(doc) else { return scene };
    let keep = find(doc, id).unwrap_or_default();
    for (path, layer) in walk(doc) {
        let related =
            keep.starts_with(&format!("{path}/")) || path == keep || path.starts_with(&format!("{keep}/"));
        let field = if layer["type"] == "sound" { "play" } else { "visible" };
        if !related && let Some(slot) = scene.pointer_mut(path.trim_start_matches("/scene")) {
            slot[field] = json!(false);
        }
    }
    scene
}
