use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MAX_DRIVEN_RIGS: usize = 64;
pub const MAX_DRIVEN_BONES: usize = 256;
pub const MAX_DRIVEN_SHAPES: usize = 128;
pub const MAX_IMPULSES: usize = 16;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RigControl {
    pub bones: BTreeMap<String, BoneSet>,
    pub shapes: BTreeMap<String, f32>,
    pub impulses: Vec<(String, [f32; 2])>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BoneSet {
    pub rotation: Option<f32>,
    pub position: Option<[f32; 2]>,
    pub scale: Option<[f32; 2]>,
    pub world: bool,
}

fn finite<const N: usize>(v: Option<[f32; N]>, bound: f32) -> Option<[f32; N]> {
    v.filter(|v| v.iter().all(|c| c.is_finite())).map(|v| v.map(|c| c.clamp(-bound, bound)))
}

impl BoneSet {
    fn bounded(self) -> BoneSet {
        BoneSet {
            rotation: finite(self.rotation.map(|r| [r]), 3600.0).map(|[r]| r),
            position: finite(self.position, 1e5),
            scale: finite(self.scale, 1000.0),
            world: self.world,
        }
    }
}

impl RigControl {
    pub fn bounded(self) -> RigControl {
        let named = |n: &String| !n.is_empty() && n.len() <= 64;
        let bones = self.bones.into_iter().filter(|(n, _)| named(n)).take(MAX_DRIVEN_BONES);
        let shapes = self.shapes.into_iter().filter(|(n, w)| named(n) && w.is_finite());
        let impulses = self.impulses.into_iter().filter(|(n, _)| named(n));
        RigControl {
            bones: bones.map(|(n, b)| (n, b.bounded())).collect(),
            shapes: shapes.take(MAX_DRIVEN_SHAPES).map(|(n, w)| (n, w.clamp(-1.0, 1.0))).collect(),
            impulses: impulses
                .filter_map(|(n, v)| Some((n, finite(Some(v), 1e5)?)))
                .take(MAX_IMPULSES)
                .collect(),
        }
    }
}
