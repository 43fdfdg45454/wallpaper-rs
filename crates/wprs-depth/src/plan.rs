use crate::brush::marks;
use crate::build::MARGIN;
use crate::guided::Size;
use crate::split::{Split, cut, distance, indices, mask, split};

pub(crate) struct Plan {
    pub size: Size,
    pub parts: Split,
    pub masks: Vec<Vec<bool>>,
    over: Vec<Vec<bool>>,
    refill: Vec<bool>,
    protect: Vec<bool>,
}

pub(crate) struct Region {
    pub known: Vec<bool>,
    pub band: Vec<bool>,
    pub reached: Vec<f32>,
    pub refill: Vec<bool>,
    pub reach: f32,
    pub far: bool,
}

fn either(a: &[bool], b: &[bool]) -> Vec<bool> {
    a.iter().zip(b).map(|(a, b)| *a || *b).collect()
}

impl Plan {
    pub fn new(levels: &[f32], size: Size, depth: &wprs_core::edit::Depth, count: usize) -> Plan {
        let parts = match &depth.cuts {
            Some(cuts) => cut(levels, cuts),
            None => split(levels, count.clamp(1, 6)),
        };
        let owner = indices(levels, &parts);
        let masks: Vec<Vec<bool>> = (0..parts.count()).map(|i| mask(&owner, i)).collect();
        let mut over = vec![vec![false; levels.len()]; parts.count()];
        for i in (1..parts.count()).rev() {
            over[i - 1] = either(&over[i], &masks[i]);
        }
        let (refill, protect) = marks(size, &depth.fills);
        Plan { size, parts, masks, over, refill, protect }
    }

    pub fn region(&self, layer: usize, reach: f32) -> Region {
        let (own, over, far) = (&self.masks[layer], &self.over[layer], layer == 0);
        let under: Vec<bool> = distance(over, self.size).iter().map(|d| *d <= MARGIN).collect();
        let mut known: Vec<bool> = own.iter().zip(&under).map(|(o, u)| *o && !*u).collect();
        if far && !known.contains(&true) {
            known.clone_from_slice(own);
        }
        let reached = distance(&known, self.size);
        let behind = |k: usize| over[k] || under[k];
        let wanted =
            |k: usize| !known[k] && !self.protect[k] && behind(k) && (reached[k] <= reach || self.refill[k]);
        let band = (0..known.len()).map(wanted).collect();
        Region { known, band, reached, refill: self.refill.clone(), reach, far }
    }
}
