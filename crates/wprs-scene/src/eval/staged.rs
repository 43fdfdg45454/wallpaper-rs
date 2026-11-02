use super::{Content, Frame};
use crate::eye::{Eye, Mat4, placement};
use crate::figure::{Coverage, Cull, Space};
use crate::resolve::Context;
use crate::{Affine, Kind, Layer, Model};
use std::ops::Range;

mod group;
mod key;
mod looks;

pub use key::seal;
pub(crate) use looks::eye;

pub const MAX_PLANES: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Look {
    pub material: u16,
    pub color: Option<[f32; 4]>,
    pub emissive: Option<[f32; 3]>,
    pub unlit: bool,
    pub lit: Option<bool>,
    pub double_sided: Option<bool>,
    pub cull: Option<Cull>,
    pub depth_test: Option<bool>,
    pub depth_write: Option<bool>,
    pub blend: Option<Coverage>,
    pub numbers: [Option<f32>; 4],
}

#[derive(Debug, Clone, PartialEq)]
pub struct Placed<'a> {
    pub layer: &'a Layer,
    pub asset: &'a str,
    pub model: &'a Model,
    pub draw: usize,
    pub matrix: Mat4,
    pub looks: Range<usize>,
    pub env: Option<&'a str>,
    pub opacity: f32,
    pub stage: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stage3d<'a> {
    pub layer: &'a str,
    pub size: [f32; 2],
    pub eye: Eye,
    pub world: Affine,
    pub origin: [f32; 2],
    pub models: Range<usize>,
    pub planes: [f32; MAX_PLANES],
    pub slices: usize,
    pub key: u64,
}

impl Stage3d<'_> {
    pub fn slice(&self, k: usize) -> [f32; 2] {
        let far = if k == 0 { f32::MAX } else { self.planes[k - 1] };
        let near = if k + 1 >= self.slices { 0.0 } else { self.planes[k] };
        [near, far]
    }
}

pub(crate) fn after<'a>(
    cx: &Context<'a>,
    layer: &'a Layer,
    (at, placed): (usize, usize),
    frame: &mut Frame<'a>,
    canvas: [f32; 2],
) {
    match &layer.kind {
        Kind::Model(m) => {
            let Some(model) = cx.scene.solids.get(&m.model) else { return };
            let vec =
                |v: &Option<crate::Value<crate::Vec3>>, or: [f32; 3]| v.as_ref().map_or(or, |v| v.get(cx).0);
            let matrix =
                placement(vec(&m.position, [0.0; 3]), vec(&m.rotation, [0.0; 3]), vec(&m.scale, [1.0; 3]));
            let start = frame.looks.len();
            looks::looks(cx, model, m, &mut frame.looks);
            let opacity = frame.draws[at].opacity;
            frame.placed.push(Placed {
                layer,
                asset: &m.model,
                model,
                draw: at,
                matrix,
                looks: start..frame.looks.len(),
                env: m.env.as_deref(),
                opacity,
                stage: usize::MAX,
            });
        }
        Kind::Group { space: Some(Space::Depth), .. } => {
            group::gather(cx, layer, (at, placed), frame, canvas)
        }
        _ => {}
    }
}

pub(crate) fn lone<'a>(frame: &mut Frame<'a>, p: usize, eye: Eye) {
    let at = frame.placed[p].draw;
    let d = &frame.draws[at];
    let Content::Model { layer, size, .. } = d.content else { return };
    let stage = frame.stages.len();
    let eye = Eye { aspect: size[0] / size[1].max(1e-3), ..eye };
    let (world, origin) = (d.world, d.origin);
    let planes = [0.0; MAX_PLANES];
    frame.stages.push(Stage3d {
        layer,
        size,
        eye,
        world,
        origin,
        models: p..p + 1,
        planes,
        slices: 1,
        key: 0,
    });
    frame.placed[p].stage = stage;
    if let Content::Model { stage: s, .. } = &mut frame.draws[at].content {
        *s = stage;
    }
}
