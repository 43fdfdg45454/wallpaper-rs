use crate::layers::{clean, taken, unique};
use crate::ops::{Op, escape, set};
use crate::rename::Change;
use crate::skin::Skin;
use crate::tidy::Tree;
use anyhow::{Result, anyhow, ensure};
use serde_json::{Value, json};
use std::path::Path;
use wprs_core::message::said;
use wprs_scene::{Cut, MAX_SKETCH_BONES, Sketch};

const MAX_SIDE: u32 = 8192;

pub struct Cutting<'a> {
    pub image: &'a str,
    pub outline: bool,
    pub cells: Option<u32>,
    pub id: Option<&'a str>,
}

pub struct Boning<'a> {
    pub layer: &'a str,
    pub name: &'a str,
    pub parent: Option<&'a str>,
    pub at: Option<[f32; 2]>,
    pub remove: bool,
    pub to: Option<&'a str>,
}

pub(crate) fn asset<'a>(doc: &'a Value, id: &str) -> Option<&'a str> {
    doc.pointer(&format!("/scene/assets/{}/path", escape(id)))?.as_str()
}

pub(crate) fn store(dir: &Path, id: &str, glb: &[u8]) -> Result<String> {
    let stash = crate::session::stash(dir).ok();
    let free = |p: &String| !dir.join(p).exists() && stash.as_ref().is_none_or(|s| !s.join(p).exists());
    let saved = (1..).map(|n| format!("meshes/{id}-{n}.glb")).find(free).unwrap_or_default();
    std::fs::create_dir_all(dir.join("meshes"))?;
    std::fs::write(dir.join(&saved), glb)?;
    Ok(saved)
}

pub(crate) fn mesh_from_picture(dir: &Path, doc: &Value, c: &Cutting) -> Result<Vec<Op>> {
    let missing =
        || anyhow!(said("editor.mesh_picture", "there is no picture \"%1\" to cut a mesh from", &[&c.image]));
    let path = asset(doc, c.image).filter(|p| crate::layers::asset_kind(p) == "image").ok_or_else(missing)?;
    let unreadable = |e: &dyn std::fmt::Display| {
        anyhow!(said("editor.unreadable_picture", "the picture cannot be read: %1", &[e]))
    };
    let (w, h) = image::image_dimensions(dir.join(path)).map_err(|e| unreadable(&e))?;
    ensure!(
        w <= MAX_SIDE && h <= MAX_SIDE,
        said("editor.mesh_side", "a mesh is cut from pictures of at most %1 pixels a side", &[&MAX_SIDE])
    );
    let picture = image::open(dir.join(path)).map_err(|e| unreadable(&e))?.into_rgba8();
    let alpha: Vec<u8> = picture.pixels().map(|p| p.0[3]).collect();
    let cut = if c.outline { Cut::Outline } else { Cut::Grid };
    let sketch = Sketch::cut(&alpha, [w, h], cut, c.cells.unwrap_or(16));
    if let Some(id) = c.id {
        ensure!(!taken(doc, id), said("editor.id_use", "the id %1 is in use", &[&id]));
    }
    let id = c.id.map_or_else(|| unique(doc, &format!("{}_mesh", clean(c.image))), String::from);
    let saved = store(dir, &id, &sketch.glb())?;
    let mut ops = set(doc, &format!("/scene/assets/{}", escape(&id)), json!({ "path": saved }));
    let len = doc.pointer("/scene/layers").and_then(Value::as_array).map_or(0, Vec::len);
    let layer = json!({ "id": id, "type": "mesh", "mesh": id, "texture": c.image, "size": [w, h] });
    ops.push(Op::Add { path: format!("/scene/layers/{len}"), value: layer });
    Ok(ops)
}

pub(crate) fn bone(dir: &Path, doc: &Value, b: &Boning) -> Result<Vec<Op>> {
    let (path, layer) = crate::actions::layer(doc, b.layer)?;
    let (id, mut skin) = Skin::load(dir, doc, layer)?;
    let (names, parents) = (skin.names.clone(), skin.parents.clone());
    let found = skin.bone(b.name).ok();
    let parent = match b.parent {
        Some("") => Some(None),
        Some(p) => Some(Some(skin.bone(p)?)),
        None => None,
    };
    let place = |s: &Skin, at: [f32; 2]| s.model(at.map(|v| v.clamp(-1.0, 2.0)));
    let change = match found {
        Some(k) if b.remove => {
            let heir = skin.up(k).map(|h| skin.names[h].clone());
            skin.remove_bone(k)?;
            let keep = heir.or_else(|| skin.bones.first().map(|h| skin.names[*h].clone()));
            Some((Change::Gone(b.name, None), keep))
        }
        None if b.remove => {
            return Err(anyhow!(said("editor.no_bone", "there is no bone \"%1\"", &[&b.name])));
        }
        Some(k) => {
            if let Some(p) = parent {
                skin.hang(k, p)?;
            }
            if let Some(at) = b.at {
                let at = place(&skin, at);
                skin.place(k, at);
            }
            match b.to.filter(|t| *t != b.name) {
                Some(to) => {
                    skin.named(to)?;
                    skin.names[k] = to.to_owned();
                    Some((Change::Renamed(b.name, to), None))
                }
                None => Some((Change::Moved, None)),
            }
        }
        None => {
            ensure!(
                skin.bones.len() < MAX_SKETCH_BONES,
                said("editor.bones_most", "a mesh has at most %1 bones here", &[&MAX_SKETCH_BONES])
            );
            let at = place(&skin, b.at.unwrap_or([0.5, 0.5]));
            skin.add_bone(b.name, parent.flatten(), at)?;
            None
        }
    };
    let before = Tree { names: &names, parents: &parents };
    let after = Tree { names: &skin.names, parents: &skin.parents };
    let ops = change.map_or_else(Vec::new, |(c, keep)| {
        let c = match c {
            Change::Gone(g, _) => Change::Gone(g, keep.as_deref()),
            c => c,
        };
        crate::rename::bone(doc, &path, c, (&before, &after))
    });
    let mut saved = skin.save(dir, doc, &id)?;
    saved.extend(ops);
    Ok(saved)
}
