use super::{Content, Frame};
use crate::eye::{Eye, Mat4, mul4, placement};
use crate::figure::{Coverage, Cull, Space};
use crate::puppet::Posed;
use crate::resolve::Context;
use crate::{Affine, Kind, Layer, Model};
use std::ops::Range;

mod group;
mod key;
mod looks;
pub mod paths;

pub use key::seal;
pub(crate) use looks::eye;
pub use looks::{CHROMA_KEY, FUR_LAYERS, FUR_LENGTH};
pub use paths::Flight;

pub const MAX_PLANES: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Shade {
    #[default]
    Pbr,
    Unlit,
    Fur,
    Foliage,
    Chroma,
    Package,
}

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
    pub shade: Shade,
    pub extra: [f32; 8],
    pub slots: [[f32; 4]; 8],
    pub package: u64,
    pub cast: Option<bool>,
    pub receive: Option<bool>,
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
    pub pose: Option<&'a Posed>,
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
    pub fade: f32,
    pub cross: Option<(Eye, f32)>,
    pub wind: [f32; 2],
    pub time: f32,
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
            let mut matrix =
                placement(vec(&m.position, [0.0; 3]), vec(&m.rotation, [0.0; 3]), vec(&m.scale, [1.0; 3]));
            let puppet = cx.puppets.iter().find(|p| p.layer == layer.id).filter(|_| model.posed());
            let pose = puppet.map(|p| &p.posed).filter(|p| p.generation > 0);
            if let Some(p) = puppet {
                let q = (frame.quality == crate::Quality::Q0).then_some(30);
                cx.paced(if p.dirty { crate::Wake::Frame } else { p.wake }, q);
            }
            if let Some(t) = pose.map(|p| p.travel).filter(|t| *t != [0.0; 3]) {
                matrix = mul4(&matrix, &placement(t, [0.0; 3], [1.0; 3]));
            }
            let start = frame.looks.len();
            looks::looks(cx, model, m, frame.quality, &mut frame.looks);
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
                pose,
            });
        }
        Kind::Group { space: Some(Space::Depth), .. } => {
            group::gather(cx, layer, (at, placed), frame, canvas)
        }
        _ => {}
    }
}

pub(crate) fn camera(cx: &Context, aspect: f32, q: crate::Quality) -> (Eye, f32, Option<(Eye, f32)>) {
    let (eye, flight) = eye(cx);
    let fix = |e: Eye| Eye { aspect, near: eye.near, far: eye.far, ..e };
    match flight {
        Some(f) => {
            if f.moving {
                cx.paced(crate::Wake::Frame, (q == crate::Quality::Q0).then_some(30));
            }
            (fix(eye), f.fade, f.cross.map(|(e, k)| (fix(e), k)))
        }
        None => (fix(eye), 0.0, None),
    }
}

pub(crate) fn lone<'a>(cx: &Context<'a>, frame: &mut Frame<'a>, p: usize) {
    let at = frame.placed[p].draw;
    let d = &frame.draws[at];
    let Content::Model { layer, size, .. } = d.content else { return };
    let stage = frame.stages.len();
    let (eye, fade, cross) = camera(cx, size[0] / size[1].max(1e-3), frame.quality);
    let (world, origin) = (d.world, d.origin);
    let planes = [0.0; MAX_PLANES];
    let (wind, time) = ([0.0; 2], cx.t);
    let models = p..p + 1;
    let slices = 1;
    frame.stages.push(Stage3d {
        layer,
        size,
        eye,
        world,
        origin,
        models,
        planes,
        slices,
        key: 0,
        fade,
        cross,
        wind,
        time,
    });
    frame.placed[p].stage = stage;
    if let Content::Model { stage: s, .. } = &mut frame.draws[at].content {
        *s = stage;
    }
}
