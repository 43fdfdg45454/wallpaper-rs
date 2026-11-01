use crate::layers::{clean, taken, unique};
use crate::ops::{Op, escape, set};
use anyhow::{Result, anyhow, ensure};
use serde_json::{Value, json};
use std::path::Path;
use wprs_core::message::said;
use wprs_scene::{Cut, MAX_SKETCH_BONES, Rigged, Sketch, SketchBone};

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
}

fn asset<'a>(doc: &'a Value, id: &str) -> Option<&'a str> {
    doc.pointer(&format!("/scene/assets/{}/path", escape(id)))?.as_str()
}

pub(crate) fn store(dir: &Path, id: &str, glb: &[u8]) -> Result<String> {
    let saved =
        (1..).map(|n| format!("meshes/{id}-{n}.glb")).find(|p| !dir.join(p).exists()).unwrap_or_default();
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

pub(crate) fn sketch_of(dir: &Path, doc: &Value, layer: &Value) -> Result<(String, Sketch)> {
    let not = || anyhow!(said("editor.not_mesh", "the layer is not a mesh", &[]));
    let id = layer["mesh"].as_str().filter(|_| layer["type"] == "mesh").ok_or_else(not)?;
    let path = asset(doc, id).ok_or_else(not)?;
    let rig = Rigged::decode(&std::fs::read(dir.join(path))?).map_err(|e| anyhow!("{path} {e}"))?;
    let canvas = doc.pointer("/scene/canvas/size").cloned().unwrap_or(json!([1920, 1080]));
    let size = layer.get("size").filter(|s| s.is_array()).unwrap_or(&canvas);
    let size = [0, 1].map(|k| size[k].as_f64().unwrap_or(1.0).max(1.0) as f32);
    let one = said(
        "editor.mesh_parts",
        "bones and depth are edited here only on meshes of one part, such as those cut from a picture",
        &[],
    );
    Ok((id.to_owned(), Sketch::from_rig(&rig, size).ok_or_else(|| anyhow!(one))?))
}

pub(crate) fn bone(dir: &Path, doc: &Value, b: &Boning) -> Result<Vec<Op>> {
    let (_, layer) = crate::actions::layer(doc, b.layer)?;
    let (id, mut sketch) = sketch_of(dir, doc, layer)?;
    let found = sketch.bones.iter().position(|x| x.name == b.name);
    let named = !b.name.is_empty() && b.name.len() <= 64 && !b.name.contains(['/', '#']);
    ensure!(named, said("editor.bone_name", "a bone has a name of 1 to 64 characters without / or #", &[]));
    let parent = b
        .parent
        .map(|p| {
            sketch
                .bones
                .iter()
                .position(|x| x.name == p)
                .ok_or_else(|| anyhow!(said("editor.no_bone", "there is no bone \"%1\"", &[&p])))
        })
        .transpose()?;
    let place =
        |at: [f32; 2]| [at[0].clamp(-1.0, 2.0) * sketch.size[0], at[1].clamp(-1.0, 2.0) * sketch.size[1]];
    match found {
        Some(k) if b.remove => {
            let up = sketch.bones[k].parent;
            sketch.bones.remove(k);
            for x in &mut sketch.bones {
                x.parent = match x.parent {
                    Some(p) if p == k => up.map(|u| if u > k { u - 1 } else { u }),
                    Some(p) if p > k => Some(p - 1),
                    p => p,
                };
            }
        }
        None if b.remove => {
            return Err(anyhow!(said("editor.no_bone", "there is no bone \"%1\"", &[&b.name])));
        }
        Some(k) => {
            let below =
                |p: usize| std::iter::successors(Some(p), |q| sketch.bones[*q].parent).any(|q| q == k);
            ensure!(
                parent.is_none_or(|p| !below(p)),
                said("editor.bone_loop", "a bone cannot hang from itself or its children", &[])
            );
            if let Some(at) = b.at {
                sketch.bones[k].at = place(at);
            }
            if b.parent.is_some() {
                sketch.bones[k].parent = parent;
            }
        }
        None => {
            ensure!(
                sketch.bones.len() < MAX_SKETCH_BONES,
                said("editor.bones_most", "a mesh has at most %1 bones here", &[&MAX_SKETCH_BONES])
            );
            let at = place(b.at.unwrap_or([0.5, 0.5]));
            sketch.bones.push(SketchBone { name: b.name.to_owned(), parent, at });
        }
    }
    sketch.bind();
    let saved = store(dir, &id, &sketch.glb())?;
    Ok(set(doc, &format!("/scene/assets/{}/path", escape(&id)), json!(saved)))
}
