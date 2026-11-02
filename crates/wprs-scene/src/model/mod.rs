mod build;
mod forge;
mod pictures;
mod posed;
mod surface;
mod tiers;

use crate::rigged::Rigged;
use crate::rigged::math::Mat;
use crate::rigged::read::{Bin, chunks};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

pub use forge::{Forge, ForgeMaterial, ForgeRig, ForgeShape};
pub use pictures::picture_size;
pub use surface::{Alpha, Finish, SLOTS};
pub use tiers::{ModelCaps, Verdict, caps, fur_shells, shadow_side, verdict};

pub const MODEL_FLOATS: usize = 12;
pub const SKIN_FLOATS: usize = 8;
pub const MAX_TEXTURE: u32 = 4096;
pub const PHONE_TEXTURE: u32 = 2048;
pub const MAX_MODEL_VERTICES: usize = 1 << 23;
pub const MAX_MODEL_TEXELS: u64 = 1 << 28;
pub const MAX_POSED_MORPHS: usize = crate::rigged::MORPH_WIDTH * 2048;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Primitive {
    pub first: u32,
    pub count: u32,
    pub material: Option<u16>,
    pub base: u32,
    pub vertices: u32,
    pub piece: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelTexture {
    pub bytes: Vec<u8>,
    pub size: [u32; 2],
}

#[derive(Debug, Clone, Default)]
pub struct Model {
    pub vertices: Vec<f32>,
    pub indices: Vec<u32>,
    pub primitives: Vec<Primitive>,
    pub materials: Vec<Finish>,
    pub textures: Vec<ModelTexture>,
    pub bounds: [[f32; 3]; 2],
    pub triangles: u64,
    pub digest: u64,
    pub paint: Vec<f32>,
    pub painted: bool,
    pub skin: Vec<f32>,
    pub morphs: Vec<[f32; 4]>,
    pub rest: Vec<Mat>,
    pub rig: Option<Arc<Rigged>>,
}

impl PartialEq for Model {
    fn eq(&self, other: &Model) -> bool {
        self.digest == other.digest
    }
}

impl Model {
    pub fn decode(bytes: &[u8]) -> Result<Model, String> {
        Model::read(bytes, false)
    }

    pub fn decode_posed(bytes: &[u8]) -> Result<Model, String> {
        Model::read(bytes, true)
    }

    fn read(bytes: &[u8], posed: bool) -> Result<Model, String> {
        let (text, bin) = chunks(bytes)?;
        let doc: crate::rigged::doc::Gltf =
            serde_json::from_slice(text).map_err(|e| format!("has invalid JSON: {e}"))?;
        if doc.required.iter().any(|e| e == "KHR_texture_transform") {
            return Err("requires KHR_texture_transform, which model layers draw without".into());
        }
        let bin = Bin { doc: &doc, bin };
        let mut h = DefaultHasher::new();
        bytes.hash(&mut h);
        posed.hash(&mut h);
        let mut model = Model { digest: h.finish(), ..Model::default() };
        let (textures, map) = pictures::textures(&bin)?;
        model.textures = textures;
        model.materials = surface::finishes(&doc, &map)?;
        match posed {
            false => build::geometry(&bin, &mut model)?,
            true => {
                let mut rig = Rigged { digest: model.digest, ..Rigged::default() };
                crate::rigged::build::joints(&doc, &mut rig)?;
                posed::geometry(&bin, &mut model, &mut rig)?;
                crate::rigged::motion::motions(&bin, &mut rig)?;
                model.rest = posed::rest(&rig);
                model.rig = Some(Arc::new(rig));
                model.bounds = posed::reach(&model);
            }
        }
        model.painted = model.paint.iter().any(|p| *p >= 0.0);
        let [lo, hi] = model.bounds;
        let tall = (hi[1] - lo[1]).max(1e-6);
        for (v, p) in model.paint.iter_mut().enumerate().filter(|(_, p)| **p < 0.0) {
            *p = ((model.vertices[v * MODEL_FLOATS + 1] - lo[1]) / tall).clamp(0.0, 1.0);
        }
        Ok(model)
    }

    pub fn skeleton(bytes: &[u8]) -> Result<Rigged, String> {
        let (text, bin) = chunks(bytes)?;
        let doc: crate::rigged::doc::Gltf =
            serde_json::from_slice(text).map_err(|e| format!("has invalid JSON: {e}"))?;
        let bin = Bin { doc: &doc, bin };
        let mut rig = Rigged::default();
        crate::rigged::build::joints(&doc, &mut rig)?;
        rig.parts = doc
            .meshes
            .iter()
            .enumerate()
            .map(|(i, m)| m.name.clone().unwrap_or(format!("mesh_{i}")))
            .collect();
        posed::skeleton(&bin, &mut rig)?;
        crate::rigged::motion::motions(&bin, &mut rig)?;
        Ok(rig)
    }

    pub fn posed(&self) -> bool {
        self.rig.is_some()
    }

    pub fn material(&self, name: &str) -> Option<usize> {
        self.materials.iter().position(|m| m.name == name)
    }

    pub fn vertex_count(&self) -> usize {
        self.vertices.len() / MODEL_FLOATS
    }

    pub fn texture_side(&self) -> u32 {
        self.textures.iter().map(|t| t.size[0].max(t.size[1])).max().unwrap_or(0)
    }

    pub fn texture_bytes(&self, cap: u32) -> u64 {
        let side = |s: u32| u64::from(s.min(cap).max(1));
        self.textures.iter().map(|t| side(t.size[0]) * side(t.size[1]) * 4 * 4 / 3).sum()
    }

    pub fn radius(&self) -> f32 {
        let [lo, hi] = self.bounds;
        let d: f32 = (0..3).map(|i| (hi[i] - lo[i]).powi(2)).sum();
        (d.sqrt() * 0.5).max(1e-3)
    }

    pub fn middle(&self) -> [f32; 3] {
        let [lo, hi] = self.bounds;
        std::array::from_fn(|i| (lo[i] + hi[i]) * 0.5)
    }
}
