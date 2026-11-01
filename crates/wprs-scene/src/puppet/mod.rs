use crate::rigged::math::{Mat, Trs};
use crate::rigged::{MAX_ACTIVE_SHAPES, Rigged};
use crate::{Kind, Layer, MeshLayer, Wake};
use std::sync::Arc;

mod cues;
mod drive;
#[cfg(test)]
mod far;
mod finish;
mod heads;
pub(crate) mod limb;
pub(crate) mod mount;
mod pose;
mod reach;
mod sway;
mod swing;
mod touch;
mod world;

pub(crate) use mount::Seat;
pub use reach::IK_ROUNDS;
pub(crate) use swing::Sim;
pub use swing::{MAX_STEPS, QUIET, QUIET_STEPS};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Drawn {
    pub piece: u16,
    pub part: u16,
    pub clip: Option<u16>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Posed {
    pub generation: u64,
    pub palette: Vec<Mat>,
    pub morphs: Vec<[(u32, f32); MAX_ACTIVE_SHAPES]>,
    pub draws: Vec<Drawn>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Head {
    pub clip: usize,
    pub layer: Option<usize>,
    pub t: f64,
    pub fade: f32,
    pub ended: bool,
    pub off: bool,
    pub held: Option<bool>,
    pub rate: Option<f32>,
    pub weight: Option<f32>,
    pub mask: Vec<bool>,
    pub speed: f32,
    pub fresh: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct ClipMap {
    pub motion: Option<usize>,
    pub bones: Vec<Option<u16>>,
    pub shapes: Vec<Option<u16>>,
    pub order: Vec<Option<u16>>,
    pub length: f32,
}

#[derive(Debug, Clone)]
pub struct Puppet {
    pub layer: String,
    pub rig: Arc<Rigged>,
    pub(crate) clips: Vec<ClipMap>,
    pub(crate) heads: Vec<Head>,
    pub(crate) order: Vec<u16>,
    pub(crate) rules: Vec<(usize, u16, u16)>,
    pub(crate) held: Vec<Option<Mat>>,
    pub(crate) locals: Vec<Trs>,
    pub(crate) worlds: Vec<Mat>,
    pub(crate) shape: Vec<f32>,
    pub(crate) sorted: Vec<Option<f32>>,
    pub(crate) buffer: Vec<f32>,
    pub(crate) next: Posed,
    pub posed: Posed,
    pub wake: Wake,
    pub(crate) dirty: bool,
    pub(crate) chains: Vec<reach::Chain>,
    pub(crate) bodies: Vec<swing::Body>,
    pub(crate) lag: f32,
    pub(crate) since: f32,
}

pub(crate) fn mesh(layer: &Layer) -> Option<&MeshLayer> {
    match &layer.kind {
        Kind::Mesh(m) => Some(m),
        _ => None,
    }
}

impl Puppet {
    pub fn new(layer: &Layer, rig: Arc<Rigged>) -> Option<Puppet> {
        let m = mesh(layer)?;
        let clips = m.rig.clips.iter().map(|c| heads::map(&rig, c)).collect();
        let heads = (0..m.rig.animations.len()).filter_map(|i| heads::head(&rig, m, Some(i), None)).collect();
        let rules: Vec<(usize, u16, u16)> = m
            .rig
            .rules
            .iter()
            .enumerate()
            .filter_map(|(k, r)| Some((k, rig.joint(&r.bone)? as u16, rig.joint(&r.parent)? as u16)))
            .collect();
        let order = world::order(&rig, &rules);
        let rules = if order.len() == rig.joints.len() { rules } else { Vec::new() };
        let order = if rules.is_empty() { rig.order.clone() } else { order };
        let n = rig.joints.len();
        let (chains, bodies) = (reach::chains(&rig, m), swing::bodies(&rig, m, &order));
        let wide = rig.motions.iter().flat_map(|m| &m.channels).map(|c| c.width()).max().unwrap_or(0);
        Some(Puppet {
            chains,
            bodies,
            lag: 0.0,
            since: 0.0,
            layer: layer.id.clone(),
            clips,
            heads,
            order,
            held: vec![None; rules.len()],
            rules,
            locals: Vec::with_capacity(n),
            worlds: Vec::with_capacity(n),
            shape: Vec::with_capacity(rig.shapes.len()),
            sorted: Vec::with_capacity(rig.parts.len()),
            buffer: vec![0.0; wide.max(rig.shapes.len()).max(4)],
            next: Posed::default(),
            posed: Posed::default(),
            wake: Wake::Frame,
            dirty: true,
            rig,
        })
    }

    pub fn posed(&self) -> &Posed {
        &self.posed
    }

    pub fn settled(&self) -> bool {
        self.bodies.iter().all(swing::Body::still)
    }

    pub(crate) fn pose(&mut self, cx: &crate::resolve::Context, m: &MeshLayer, (seat, sim): (&Seat, &Sim)) {
        self.blend(cx, m);
        self.worlds(cx, m);
        if !self.chains.is_empty() {
            self.reach(cx, m, seat);
        }
        if let Some(set) = cx.script.rigs.get(&self.layer) {
            self.drive(cx, m, set, seat);
        }
        if !self.bodies.is_empty() {
            self.swing(cx, m, sim);
        }
        self.finish(cx, m);
    }
}
