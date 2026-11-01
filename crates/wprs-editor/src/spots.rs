use crate::action::PointEdit;
use crate::ops::Op;
use crate::rename::Change;
use crate::skin::Skin;
use crate::tidy::Tree;
use anyhow::{Result, anyhow, ensure};
use serde_json::Value;
use std::path::Path;
use wprs_core::message::said;

pub(crate) fn point(dir: &Path, doc: &Value, p: &PointEdit) -> Result<Vec<Op>> {
    let (path, layer) = crate::actions::layer(doc, &p.layer)?;
    let (id, mut skin) = Skin::load(dir, doc, layer)?;
    let (names, parents) = (skin.names.clone(), skin.parents.clone());
    let parent = match p.parent.as_deref() {
        Some("") => Some(None),
        Some(b) => Some(Some(skin.bone(b)?)),
        None => None,
    };
    let fine = p.at.is_none_or(|a| a.iter().all(|v| v.is_finite()));
    ensure!(fine, said("editor.point_at", "a point is placed in fractions of the layer", &[]));
    let at = p.at.map(|a| skin.model(a.map(|v| v.clamp(-1.0, 2.0))));
    let change = match skin.point(&p.name) {
        Ok(k) if p.remove => {
            skin.remove_bone(k)?;
            Change::Gone(&p.name, None)
        }
        Err(e) if p.remove => return Err(e),
        Ok(k) => {
            if let Some(up) = parent {
                skin.hang(k, up)?;
            }
            if let Some(at) = at {
                skin.place(k, at);
            }
            match p.to.as_deref().filter(|t| *t != p.name) {
                Some(to) => {
                    skin.named(to)?;
                    skin.names[k] = to.to_owned();
                    Change::Renamed(&p.name, to)
                }
                None => Change::Moved,
            }
        }
        Err(_) => {
            let missing = || anyhow!(said("editor.point_where", "say where the new point goes", &[]));
            let k = skin.add_point(&p.name, at.ok_or_else(missing)?)?;
            if let Some(up) = parent {
                skin.hang(k, up)?;
            }
            Change::Moved
        }
    };
    let before = Tree { names: &names, parents: &parents };
    let after = Tree { names: &skin.names, parents: &skin.parents };
    let ops = crate::rename::bone(doc, &path, change, (&before, &after));
    let mut saved = skin.save(dir, doc, &id)?;
    saved.extend(ops);
    Ok(saved)
}
