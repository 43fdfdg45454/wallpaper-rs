use crate::{Kind, Layer, MeshLayer, MeshPart, Rig};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Fig<'a> {
    pub rig: &'a Rig,
    pub parts: &'a BTreeMap<String, MeshPart>,
    pub size: Option<[f32; 2]>,
    pub unit: f32,
    pub root: bool,
    pub sort: bool,
}

static NO_PARTS: BTreeMap<String, MeshPart> = BTreeMap::new();
pub const MODEL_UNIT: f32 = 0.01;

pub(crate) fn mesh(layer: &Layer) -> Option<&MeshLayer> {
    match &layer.kind {
        Kind::Mesh(m) => Some(m),
        _ => None,
    }
}

pub(crate) fn figure(layer: &Layer) -> Option<Fig<'_>> {
    match &layer.kind {
        Kind::Mesh(m) => {
            let sort = m.depth.is_some_and(|d| d.sort);
            Some(Fig { rig: &m.rig, parts: &m.parts, size: m.size, unit: 1.0, root: false, sort })
        }
        Kind::Model(m) if !m.rig.is_empty() || m.root_motion => {
            let (parts, size, root) = (&NO_PARTS, m.size, m.root_motion);
            Some(Fig { rig: &m.rig, parts, size, unit: MODEL_UNIT, root, sort: false })
        }
        _ => None,
    }
}
