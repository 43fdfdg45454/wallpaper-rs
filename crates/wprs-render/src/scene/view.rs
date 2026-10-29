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
    pub(super) texts: HashMap<u64, (GpuImage, [f32; 2], u64)>,
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
    pub(super) scratch: super::Scratch,
    pub(super) marks: Vec<super::damage::Mark>,
    pub(super) stamp: u64,
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

    pub fn stats(&self) -> SceneStats {
        self.stats
    }

    pub fn trim(&mut self) {
        self.marks.clear();
        self.pool.trim();
        self.states.clear();
        self.swarms.iter_mut().for_each(swarm::Swarms::trim);
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
