mod build;
mod cut;
mod delaunay;
mod doc;
mod geometry;
pub mod math;
mod motion;
mod read;
mod sketch;
mod write;

use math::{Mat, Trs};
use std::hash::{DefaultHasher, Hash, Hasher};

pub use cut::{Cut, MAX_CELLS, MIN_CELLS};
pub use motion::{Channel, Interp, Motion, Path};
pub use sketch::{MAX_SKETCH_BONES, Sketch, SketchBone};

pub const VERTEX_FLOATS: usize = 14;
pub const MAX_RIG_VERTICES: usize = 65_536;
pub const MAX_RIG_PALETTE: usize = 256;
pub const MAX_RIG_PIECES: usize = 64;
pub const MORPH_WIDTH: usize = 2048;
pub const MAX_MORPH_TEXELS: usize = MORPH_WIDTH * 512;
pub const MAX_RIG_BYTES: usize = 128 << 20;
pub const MAX_RIG_KEYS: usize = 1 << 22;
pub const MAX_ACTIVE_SHAPES: usize = 8;

#[derive(Debug, Clone, PartialEq)]
pub struct Joint {
    pub name: String,
    pub parent: Option<u16>,
    pub rest: Trs,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Piece {
    pub part: u16,
    pub first: u32,
    pub count: u32,
    pub base: u32,
    pub vertices: u32,
    pub targets: Vec<(u16, u32)>,
    pub color: [f32; 4],
}

#[derive(Debug, Clone, Default)]
pub struct Rigged {
    pub joints: Vec<Joint>,
    pub order: Vec<u16>,
    pub palette: Vec<(u16, Mat)>,
    pub parts: Vec<String>,
    pub pieces: Vec<Piece>,
    pub vertices: Vec<f32>,
    pub indices: Vec<u32>,
    pub morphs: Vec<[f32; 4]>,
    pub shapes: Vec<String>,
    pub defaults: Vec<f32>,
    pub motions: Vec<Motion>,
    pub picture: Option<Vec<u8>>,
    pub digest: u64,
}

impl PartialEq for Rigged {
    fn eq(&self, other: &Rigged) -> bool {
        self.digest == other.digest
    }
}

impl Rigged {
    pub fn decode(bytes: &[u8]) -> Result<Rigged, String> {
        let (text, bin) = read::chunks(bytes)?;
        let doc: doc::Gltf = serde_json::from_slice(text).map_err(|e| format!("has invalid JSON: {e}"))?;
        let bin = read::Bin { doc: &doc, bin };
        let mut rig = Rigged { digest: digest(bytes), ..Rigged::default() };
        build::joints(&doc, &mut rig)?;
        geometry::pieces(&bin, &mut rig)?;
        motion::motions(&bin, &mut rig)?;
        rig.picture = geometry::picture(&bin);
        Ok(rig)
    }

    pub fn joint(&self, name: &str) -> Option<usize> {
        self.joints.iter().position(|j| j.name == name)
    }

    pub fn part(&self, name: &str) -> Option<usize> {
        self.parts.iter().position(|p| p == name)
    }

    pub fn shape(&self, name: &str) -> Option<usize> {
        self.shapes.iter().position(|s| s == name)
    }

    pub fn motion(&self, name: Option<&str>) -> Option<&Motion> {
        match name {
            Some(n) => self.motions.iter().find(|m| m.name == n),
            None => self.motions.first(),
        }
    }

    pub fn descends(&self, node: usize, from: usize) -> bool {
        std::iter::successors(Some(node), |n| self.joints[*n].parent.map(usize::from))
            .take(self.joints.len() + 1)
            .any(|n| n == from)
    }

    pub fn vertex_count(&self) -> usize {
        self.vertices.len() / VERTEX_FLOATS
    }
}

fn digest(bytes: &[u8]) -> u64 {
    let mut h = DefaultHasher::new();
    bytes.hash(&mut h);
    h.finish()
}
