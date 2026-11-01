use super::limb::{end, pivot, tip};
use crate::rigged::Rigged;
use crate::{Body as Mode, MeshLayer};

pub const MAX_STEPS: u32 = 15;
pub const QUIET_STEPS: u32 = 30;
pub const QUIET: f32 = 0.005;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Body {
    pub entry: usize,
    pub mode: Mode,
    pub nodes: Vec<u16>,
    pub tips: Vec<[f32; 3]>,
    pub at: Vec<[f32; 2]>,
    pub was: Vec<[f32; 2]>,
    pub pins: Vec<[f32; 2]>,
    pub before: Vec<[f32; 2]>,
    pub fresh: bool,
    pub quiet: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Sim {
    pub frozen: bool,
    pub h: f32,
    pub wind: [f32; 2],
    pub t: f32,
}

pub(super) fn bodies(rig: &Rigged, m: &MeshLayer, order: &[u16]) -> Vec<Body> {
    let own = |n: usize| m.rig.physics.iter().any(|p| rig.joint(&p.bone) == Some(n));
    let mut out: Vec<Body> = m
        .rig
        .physics
        .iter()
        .enumerate()
        .filter_map(|(entry, p)| {
            let mut nodes = vec![rig.joint(&p.bone)? as u16];
            while p.mode == Mode::Rope && nodes.len() < crate::MAX_CHAIN {
                let last = *nodes.last()?;
                let next = rig.joints.iter().position(|j| j.parent == Some(last));
                match next.filter(|c| !own(*c)) {
                    Some(c) => nodes.push(c as u16),
                    None => break,
                }
            }
            let tips: Vec<[f32; 3]> = nodes
                .iter()
                .enumerate()
                .map(|(i, &n)| {
                    nodes.get(i + 1).map_or_else(|| tip(rig, n as usize), |c| rig.joints[*c as usize].rest.t)
                })
                .collect();
            let points = vec![[0.0; 2]; nodes.len() + 1];
            let (at, was, pins, before) = (points.clone(), points.clone(), points.clone(), points);
            Some(Body { entry, mode: p.mode, nodes, tips, at, was, pins, before, fresh: true, quiet: 0 })
        })
        .take(crate::MAX_BODIES)
        .collect();
    let rank = |b: &Body| order.iter().position(|n| *n == b.nodes[0]).unwrap_or(usize::MAX);
    out.sort_by_key(rank);
    out
}

impl Body {
    pub(crate) fn pin(&mut self, worlds: &[crate::rigged::math::Mat]) {
        self.pins[0] = pivot(&worlds[self.nodes[0] as usize]);
        for (i, &n) in self.nodes.iter().enumerate() {
            self.pins[i + 1] = end(&worlds[n as usize], self.tips[i]);
        }
        if self.fresh {
            self.at.clone_from_slice(&self.pins);
            self.was.clone_from_slice(&self.pins);
            self.before.clone_from_slice(&self.pins);
            self.fresh = false;
        }
    }

    pub(crate) fn hold(&mut self) {
        self.at.clone_from_slice(&self.pins);
        self.was.clone_from_slice(&self.pins);
        self.before.clone_from_slice(&self.pins);
        self.quiet = QUIET_STEPS;
    }

    pub(crate) fn still(&self) -> bool {
        self.quiet >= QUIET_STEPS && self.before == self.pins
    }

    pub(crate) fn push(&mut self, node: u16, v: [f32; 2], h: f32) {
        let Some(i) = self.nodes.iter().position(|n| *n == node) else { return };
        let p = &mut self.was[i + 1];
        (p[0], p[1]) = (p[0] - v[0] * h, p[1] - v[1] * h);
        self.quiet = 0;
    }
}
