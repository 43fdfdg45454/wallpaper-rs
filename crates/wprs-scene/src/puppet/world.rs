use super::Puppet;
use crate::MeshLayer;
use crate::resolve::Context;
use crate::rigged::Rigged;
use crate::rigged::math::{IDENTITY, decompose, invert, mul};

pub(super) fn order(rig: &Rigged, rules: &[(usize, u16, u16)]) -> Vec<u16> {
    let n = rig.joints.len();
    let mut after: Vec<Vec<u16>> = vec![Vec::new(); n];
    let mut need = vec![0usize; n];
    let edges = rig.joints.iter().enumerate().filter_map(|(i, j)| Some((j.parent?, i as u16)));
    for (from, to) in edges.chain(rules.iter().map(|r| (r.2, r.1))) {
        after[from as usize].push(to);
        need[to as usize] += 1;
    }
    let mut out: Vec<u16> = (0..n).filter(|i| need[*i] == 0).map(|i| i as u16).collect();
    let mut at = 0;
    while let Some(&k) = out.get(at) {
        for &c in &after[k as usize] {
            need[c as usize] -= 1;
            if need[c as usize] == 0 {
                out.push(c);
            }
        }
        at += 1;
    }
    out
}

impl Puppet {
    pub(crate) fn worlds(&mut self, cx: &Context, m: &MeshLayer) {
        let Puppet { rig, order, rules, held, locals, worlds, .. } = self;
        worlds.clear();
        worlds.resize(rig.joints.len(), IDENTITY);
        for &n in order.iter() {
            let n = n as usize;
            let local = locals[n].matrix();
            let mut w = rig.joints[n].parent.map_or(local, |p| mul(&worlds[p as usize], &local));
            for (j, &(k, _, to)) in rules.iter().enumerate().filter(|(_, r)| r.1 as usize == n) {
                let weight = m.rig.rules[k].weight.get(cx).clamp(0.0, 1.0);
                if weight <= 0.0 {
                    held[j] = None;
                    continue;
                }
                let to = &worlds[to as usize];
                let offset = *held[j].get_or_insert_with(|| mul(&invert(to), &w));
                w = decompose(&w).lerp(&decompose(&mul(to, &offset)), weight).matrix();
            }
            worlds[n] = w;
        }
    }
}
