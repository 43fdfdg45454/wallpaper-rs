mod build;
mod forge;
mod pictures;
mod surface;
mod tiers;

use crate::rigged::read::{Bin, chunks};
use std::hash::{DefaultHasher, Hash, Hasher};

pub use forge::{Forge, ForgeMaterial, ForgeShape};
pub use pictures::picture_size;
pub use surface::{Alpha, Finish, SLOTS};
pub use tiers::{ModelCaps, Verdict, caps, verdict};

pub const MODEL_FLOATS: usize = 12;
pub const MAX_TEXTURE: u32 = 4096;
pub const PHONE_TEXTURE: u32 = 2048;
pub const MAX_MODEL_VERTICES: usize = 1 << 23;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Primitive {
    pub first: u32,
    pub count: u32,
    pub material: Option<u16>,
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
}

impl PartialEq for Model {
    fn eq(&self, other: &Model) -> bool {
        self.digest == other.digest
    }
}

impl Model {
    pub fn decode(bytes: &[u8]) -> Result<Model, String> {
        let (text, bin) = chunks(bytes)?;
        let doc: crate::rigged::doc::Gltf =
            serde_json::from_slice(text).map_err(|e| format!("has invalid JSON: {e}"))?;
        let bin = Bin { doc: &doc, bin };
        let mut h = DefaultHasher::new();
        bytes.hash(&mut h);
        let mut model = Model { digest: h.finish(), ..Model::default() };
        model.textures = pictures::textures(&bin)?;
        model.materials = surface::finishes(&doc, model.textures.len())?;
        build::geometry(&bin, &mut model)?;
        Ok(model)
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
