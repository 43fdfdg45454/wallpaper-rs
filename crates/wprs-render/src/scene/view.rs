use super::chains::Lease;
use super::pool::{Pool, Tex};
use super::state::Sim;
use super::{Cached, Image, SceneStats, fonts, swarm};
use crate::texture::GpuImage;
use std::collections::HashMap;
use std::sync::Arc;
use wgpu::TextureFormat as Format;
use wprs_core::{Fit, UvRect};
use wprs_scene::Affine;

pub struct SceneView {
    pub(super) fit: Fit,
    pub(super) canvas: [u32; 2],
    pub(super) srgb: bool,
    pub(super) format: Format,
    pub(super) images: HashMap<String, Image>,
    pub(super) samplings: HashMap<String, super::sampling::Sampling>,
    pub(super) sources: HashMap<String, Arc<super::Pixels>>,
    pub(super) fonts: fonts::Fonts,
    pub(super) texts: HashMap<u64, super::rasters::Raster>,
    pub(super) pool: Pool,
    pub(super) caches: HashMap<usize, Cached>,
    pub(super) luts: Vec<(String, Tex)>,
    pub(super) states: HashMap<usize, Sim>,
    pub(super) state_format: Format,
    pub(super) target: Option<(Tex, GpuImage)>,
    pub(super) uniform: wgpu::Buffer,
    pub(super) stats: SceneStats,
    pub(super) swarms: Option<swarm::Swarms>,
    pub(super) _lease: Lease,
    pub(super) refused: std::collections::BTreeMap<String, String>,
    pub(super) scratch: super::Scratch,
    pub(super) marks: Vec<super::damage::Mark>,
    pub(super) stamp: u64,
    pub(super) trimmed: bool,
    pub(super) region: Option<wprs_core::Region>,
    pub(super) shrink: (f32, u32),
    pub(super) id: usize,
    pub(super) paints: Vec<(String, String, Image)>,
    pub(super) clock: super::clock::Clock,
    pub(super) hitboxes: wprs_scene::Hitboxes,
    pub(super) covers: HashMap<String, Option<Arc<wprs_scene::AlphaMap>>>,
    pub(super) molds: HashMap<String, super::molds::Mold>,
    pub(super) skins: HashMap<String, super::skins::Skin>,
}

impl Drop for SceneView {
    fn drop(&mut self) {
        super::flight::forget(self.id);
    }
}

impl SceneView {
    pub fn view(&self, size: (u32, u32)) -> [f32; 4] {
        let fit = match self.fit {
            Fit::Tile => Fit::Center,
            Fit::Span => Fit::Fill,
            fit => fit,
        };
        let uv = UvRect::compute(fit, (self.canvas[0], self.canvas[1]), size);
        let [cw, ch] = self.canvas.map(|v| v as f32);
        [uv.offset[0] * cw, uv.offset[1] * ch, uv.scale[0] * cw, uv.scale[1] * ch]
    }

    pub fn left(&self) -> Vec<(String, String)> {
        let short = |why: &String| {
            why.lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or_default()
                .chars()
                .take(240)
                .collect::<String>()
        };
        self.refused.iter().map(|(id, why)| (id.clone(), short(why))).collect()
    }

    pub fn left_out(&self) -> Vec<String> {
        self.left().into_iter().map(|(id, why)| format!("the effect {id} was left out: {why}")).collect()
    }

    pub fn left_key(&self) -> u64 {
        use std::hash::{DefaultHasher, Hash, Hasher};
        let mut h = DefaultHasher::new();
        self.refused.hash(&mut h);
        if self.refused.is_empty() { 0 } else { h.finish() | 1 }
    }

    pub fn retry(&mut self) -> bool {
        self.marks.clear();
        self.pool.glow.baked = None;
        let pool = &mut self.pool;
        self.caches.drain().for_each(|(_, c)| pool.give(c.tex));
        let again = !self.refused.is_empty();
        self.refused.clear();
        again
    }

    pub fn stats(&self) -> SceneStats {
        self.stats
    }

    pub fn trim(&mut self) {
        self.trimmed = true;
        self.pool.trim();
        self.states.clear();
        self.swarms.iter_mut().for_each(swarm::Swarms::trim);
    }

    pub fn damage(&self) -> Option<wprs_core::Region> {
        self.region
    }

    pub fn image(&self) -> Option<&GpuImage> {
        self.target.as_ref().map(|(_, image)| image)
    }

    pub(super) fn screen(&self, size: (u32, u32)) -> Affine {
        let [x, y, w, h] = self.view(size);
        let (sx, sy) = (size.0 as f32 / w, size.1 as f32 / h);
        Affine([sx, 0.0, 0.0, sy, -x * sx, -y * sy])
    }
}
