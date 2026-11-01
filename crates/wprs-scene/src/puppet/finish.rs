use super::{Drawn, Puppet};
use crate::MeshLayer;
use crate::resolve::Context;
use crate::rigged::MAX_ACTIVE_SHAPES;
use crate::rigged::math::mul;

impl Puppet {
    pub(crate) fn finish(&mut self, cx: &Context, m: &MeshLayer) {
        let Puppet { rig, worlds, shape, sorted, next, posed, .. } = self;
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
