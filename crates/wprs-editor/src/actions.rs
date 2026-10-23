use crate::arrange;
use crate::keys::{Key, keyframe};
use crate::layers::{children, clean, find, parent, system, taken, template, unique, walk};
use crate::ops::{Op, set};
use anyhow::{Result, anyhow, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;
use wprs_format::{Descriptor, Rating};

pub const EDITOR: &str = "/scene/ext/editor";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Set {
        pointer: String,
        value: Value,
    },
    Add {
        pointer: String,
        value: Value,
    },
    Remove {
        pointer: String,
    },
    Move {
        from: String,
        to: String,
    },
    AddLayer {
        kind: String,
        #[serde(default)]
        id: Option<String>,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        source: Option<String>,
        #[serde(default)]
        into: Option<String>,
        #[serde(default)]
        at: Option<usize>,
    },
    RemoveLayer {
        id: String,
    },
    DuplicateLayer {
        id: String,
    },
    MoveLayer {
        id: String,
        to: usize,
        #[serde(default)]
        into: Option<String>,
    },
    Group {
        ids: Vec<String>,
        #[serde(default)]
        id: Option<String>,
    },
    Ungroup {
        id: String,
    },
    Hide {
        id: String,
        hidden: bool,
    },
    Lock {
        id: String,
        locked: bool,
    },
    Solo {
        #[serde(default)]
        id: Option<String>,
    },
    Keyframe {
        pointer: String,
        time: f64,
        #[serde(default)]
        value: Option<Value>,
        #[serde(default)]
        ease: Option<Value>,
        #[serde(default)]
        timeline: Option<String>,
        #[serde(default)]
        remove: bool,
    },
    Import {
        file: PathBuf,
        #[serde(default)]
        id: Option<String>,
    },
    Translate {
        pointer: String,
        language: String,
        #[serde(default)]
        text: Option<String>,
        #[serde(default)]
        add: bool,
    },
    Thumbnail {
        file: PathBuf,
        #[serde(default)]
        animated: bool,
    },
    Upgrade {
        #[serde(default)]
        rating: Option<Rating>,
        #[serde(default)]
        reasons: Vec<Descriptor>,
        #[serde(default)]
        photosensitive: Option<bool>,
        #[serde(default)]
        derived_from: Vec<String>,
    },
    AddProperty {
        kind: String,
        #[serde(default)]
        id: Option<String>,
        #[serde(default)]
        group: Option<String>,
        #[serde(default)]
        label: Option<String>,
    },
    MoveProperty {
        id: String,
        to: usize,
        #[serde(default)]
        group: Option<String>,
    },
    RenameProperty {
        id: String,
        to: String,
    },
    RemoveProperty {
        id: String,
    },
    Condition {
        id: String,
        #[serde(default)]
        enabled: bool,
        #[serde(default)]
        text: String,
    },
    AddPreset {
        #[serde(default)]
        id: Option<String>,
        #[serde(default)]
        label: Option<String>,
    },
    Undo {},
    Redo {},
}

pub const ACTIONS: [&str; 26] = [
    "set",
    "add",
    "remove",
    "move",
    "add_layer",
    "remove_layer",
    "duplicate_layer",
    "move_layer",
    "group",
    "ungroup",
    "hide",
    "lock",
    "solo",
    "keyframe",
    "import",
    "translate",
    "thumbnail",
    "upgrade",
    "add_property",
    "move_property",
    "rename_property",
    "remove_property",
    "condition",
    "add_preset",
    "undo",
    "redo",
];

impl Action {
    pub fn name(&self) -> String {
        serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_object()?.keys().next().cloned())
            .unwrap_or_default()
    }
}

pub fn layer<'a>(doc: &'a Value, id: &str) -> Result<(String, &'a Value)> {
    let path = find(doc, id)?;
    Ok((path.clone(), doc.pointer(&path).ok_or_else(|| anyhow!("there is no layer {id:?}"))?))
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

fn computed(pointer: &str) -> Result<()> {
    let typed = pointer.starts_with("/manifest/requires/features");
    ensure!(!typed, "requires.features is computed from what the package uses");
    Ok(())
}

pub fn plan(doc: &Value, action: &Action) -> Result<Vec<Op>> {
    if let Action::Set { pointer, .. }
    | Action::Add { pointer, .. }
    | Action::Remove { pointer }
    | Action::Translate { pointer, .. } = action
    {
        computed(pointer)?;
    }
    if let Action::Move { from, to } = action {
        computed(from).and(computed(to))?;
    }
    Ok(match action {
        Action::Set { pointer, value } => set(doc, pointer, value.clone()),
        Action::Add { pointer, value } => vec![Op::Add { path: pointer.clone(), value: value.clone() }],
        Action::Remove { pointer } => vec![Op::Remove { path: pointer.clone() }],
        Action::Move { from, to } => vec![Op::Move { from: from.clone(), path: to.clone() }],
        Action::AddLayer { kind, id, name, source, into, at } => {
            if let Some(id) = id {
                ensure!(!taken(doc, id), "the id {id} is in use");
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
        Action::Keyframe { pointer, time, value, ease, timeline, remove } => {
            let key = Key {
                time: *time,
                value: value.clone(),
                ease: ease.clone(),
                timeline: timeline.as_deref(),
                remove: *remove,
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
        Action::Import { .. } | Action::Thumbnail { .. } | Action::Undo {} | Action::Redo {} => Vec::new(),
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
