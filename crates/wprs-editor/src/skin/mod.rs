mod bind;
mod bones;
mod brush;
mod glb;
mod points;
mod read;
mod write;

use crate::ops::{Op, escape, set};
use anyhow::{Result, anyhow};
use serde_json::{Value, json};
use std::path::Path;
use wprs_core::message::said;
use wprs_scene::mesh_math::{Mat, apply, mul};
use wprs_scene::{Rigged, Trs};

pub use brush::{Brush, Mode};
pub(crate) use glb::Glb;

pub(crate) struct Piece {
    pub node: usize,
    pub mesh: usize,
    pub primitive: usize,
    pub first: usize,
    pub count: usize,
    pub targets: Vec<(usize, Vec<[f32; 3]>)>,
}

pub(crate) struct Skin {
    pub size: [f32; 2],
    pub glb: Glb,
    pub names: Vec<String>,
    pub parents: Vec<Option<usize>>,
    pub locals: Vec<Trs>,
    pub bones: Vec<usize>,
    pub points: Vec<[f32; 3]>,
    pub weights: Vec<Vec<(usize, f32)>>,
    pub loose: Vec<bool>,
    pub triangles: Vec<[u32; 3]>,
    pub pieces: Vec<Piece>,
    pub auto: bool,
}

pub(crate) fn rigged(dir: &Path, doc: &Value, layer: &Value) -> Result<(String, String, Rigged)> {
    let not = || anyhow!(said("editor.not_mesh", "the layer is not a mesh", &[]));
    let id = layer["mesh"].as_str().filter(|_| layer["type"] == "mesh").ok_or_else(not)?;
    let path = crate::rig::asset(doc, id).ok_or_else(not)?;
    let rig = Rigged::decode(&std::fs::read(dir.join(path))?).map_err(|e| anyhow!("{path} {e}"))?;
    Ok((id.to_owned(), path.to_owned(), rig))
}

pub(crate) fn size(doc: &Value, layer: &Value) -> [f32; 2] {
    let canvas = doc.pointer("/scene/canvas/size").cloned().unwrap_or(json!([1920, 1080]));
    let size = layer.get("size").filter(|s| s.is_array()).unwrap_or(&canvas);
    [0, 1].map(|k| size[k].as_f64().unwrap_or(1.0).max(1.0) as f32)
}

impl Skin {
    pub fn load(dir: &Path, doc: &Value, layer: &Value) -> Result<(String, Skin)> {
        let (id, path, rig) = rigged(dir, doc, layer)?;
        let glb = Glb::read(&std::fs::read(dir.join(&path))?)?;
        Ok((id, Skin::from(glb, &rig, size(doc, layer))?))
    }

    pub fn save(self, dir: &Path, doc: &Value, id: &str) -> Result<Vec<Op>> {
        let saved = crate::rig::store(dir, id, &self.into_glb()?)?;
        Ok(set(doc, &format!("/scene/assets/{}/path", escape(id)), json!(saved)))
    }

    pub fn worlds(&self) -> Vec<Mat> {
        let depth = |n: usize| std::iter::successors(Some(n), |p| self.parents[*p]).count();
        let mut order: Vec<usize> = (0..self.names.len()).collect();
        order.sort_by_key(|n| depth(*n));
        let mut out = vec![wprs_scene::mesh_math::IDENTITY; self.names.len()];
        for n in order {
            let local = self.locals[n].matrix();
            out[n] = self.parents[n].map_or(local, |p| mul(&out[p], &local));
        }
        out
    }

    pub fn picture(&self, [x, y, _]: [f32; 3]) -> [f32; 2] {
        [(x + self.size[0] / 2.0) / self.size[0], (self.size[1] / 2.0 - y) / self.size[1]]
    }

    pub fn model(&self, [fx, fy]: [f32; 2]) -> [f32; 2] {
        [fx * self.size[0] - self.size[0] / 2.0, self.size[1] / 2.0 - fy * self.size[1]]
    }

    pub fn head(&self, worlds: &[Mat], node: usize) -> [f32; 3] {
        apply(&worlds[node], [0.0; 3])
    }

    pub fn bone(&self, name: &str) -> Result<usize> {
        let found = self.bones.iter().copied().find(|b| self.names[*b] == name);
        found.ok_or_else(|| anyhow!(said("editor.no_bone", "there is no bone \"%1\"", &[&name])))
    }

    pub fn up(&self, node: usize) -> Option<usize> {
        std::iter::successors(self.parents[node], |p| self.parents[*p]).find(|p| self.bones.contains(p))
    }
}
