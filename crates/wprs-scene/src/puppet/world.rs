use super::{Drawn, Puppet};
use crate::MeshLayer;
use crate::resolve::Context;
use crate::rigged::math::{IDENTITY, decompose, invert, mul};
use crate::rigged::{MAX_ACTIVE_SHAPES, Rigged};

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
    pub(crate) fn place(&mut self, cx: &Context, m: &MeshLayer) {
        let Puppet { rig, order, rules, held, locals, worlds, shape, sorted, next, posed, .. } = self;
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
        next.palette.clear();
        next.palette.extend(rig.palette.iter().map(|(node, inverse)| mul(&worlds[*node as usize], inverse)));
        next.morphs.clear();
        for piece in &rig.pieces {
            let mut slots = [(0u32, 0.0f32); MAX_ACTIVE_SHAPES];
            for &(s, base) in &piece.targets {
                let w = shape[s as usize];
                let weakest =
                    (0..MAX_ACTIVE_SHAPES).min_by(|a, b| slots[*a].1.abs().total_cmp(&slots[*b].1.abs()));
                match weakest {
                    Some(k) if w != 0.0 && w.abs() > slots[k].1.abs() => slots[k] = (base, w),
                    _ => {}
                }
            }
            next.morphs.push(slots);
        }
        for (name, part) in &m.parts {
            let Some(p) = rig.part(name) else { continue };
            if let Some(o) = &part.order {
                sorted[p] = Some(o.get(cx).clamp(-1000.0, 1000.0));
            }
        }
        next.draws.clear();
        for (i, piece) in rig.pieces.iter().enumerate() {
            let part = m.parts.get(&rig.parts[piece.part as usize]);
            if part.and_then(|p| p.visible.as_ref()).is_some_and(|v| !v.get(cx)) {
                continue;
            }
            let clip = part.and_then(|p| rig.part(p.clip_to.as_deref()?)).map(|c| c as u16);
            next.draws.push(Drawn { piece: i as u16, part: piece.part, clip });
        }
        let key = |d: &Drawn| sorted[d.part as usize].unwrap_or(0.0);
        next.draws.sort_unstable_by(|a, b| key(a).total_cmp(&key(b)).then(a.piece.cmp(&b.piece)));
        let same = next.palette == posed.palette && next.morphs == posed.morphs && next.draws == posed.draws;
        if !same || posed.generation == 0 {
            next.generation = posed.generation + 1;
            std::mem::swap(next, posed);
        }
    }
}
