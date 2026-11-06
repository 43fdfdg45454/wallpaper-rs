use super::{Op, Src};

fn writes(op: &Op) -> Option<Src> {
    match op {
        Op::Clear(t, _)
        | Op::Draw { target: t, .. }
        | Op::Lit { target: t, .. }
        | Op::Particles { target: t, .. } => Some(*t),
        Op::Effect(f) => Some(f.target),
        Op::Copy { to, .. } => Some(*to),
        Op::Read { .. } | Op::Strip { .. } => None,
    }
}

fn reads(op: &Op, sets: &[[Src; 11]], s: Src) -> bool {
    match op {
        Op::Clear(..) => false,
        Op::Copy { from, .. } | Op::Read { from, .. } | Op::Strip { from } => *from == s,
        Op::Draw { target, tex, .. } => *target == s || tex.contains(&s),
        Op::Lit { target, tex, maps, .. } => *target == s || tex.contains(&s) || maps.contains(&s),
        Op::Effect(f) => f.load && f.target == s || sets.get(f.tex as usize).is_none_or(|t| t.contains(&s)),
        Op::Particles { target, .. } => *target == s,
    }
}

fn fixed(op: &Op) -> bool {
    matches!(op, Op::Read { .. } | Op::Strip { .. })
}

fn apart(a: &Op, b: &Op, sets: &[[Src; 11]]) -> bool {
    let touches =
        |w: Option<Src>, other: &Op| w.is_some_and(|w| reads(other, sets, w) || writes(other) == Some(w));
    !fixed(a) && !fixed(b) && !touches(writes(a), b) && !touches(writes(b), a)
}

fn joins(op: &Op, target: Src) -> bool {
    matches!(op, Op::Draw { .. } | Op::Lit { .. } | Op::Particles { .. }) && writes(op) == Some(target)
}

pub(crate) fn gather(ops: &mut [Op], sets: &[[Src; 11]]) {
    let mut i = 0;
    while i < ops.len() {
        let Some(t) = writes(&ops[i]).filter(|t| matches!(ops[i], Op::Clear(..)) || joins(&ops[i], *t))
        else {
            i += 1;
            continue;
        };
        let end = (i + 1..ops.len()).find(|k| !joins(&ops[*k], t)).unwrap_or(ops.len());
        let touched = |op: &Op| fixed(op) || writes(op) == Some(t) || reads(op, sets, t);
        let Some(next) = (end..ops.len()).find(|k| touched(&ops[*k])).filter(|k| joins(&ops[*k], t)) else {
            i = end;
            continue;
        };
        let (mut at, mut stuck) = (i, false);
        for k in end..next {
            if (at..k).all(|m| apart(&ops[m], &ops[k], sets)) {
                ops[at..=k].rotate_right(1);
                at += 1;
            } else {
                stuck = true;
            }
        }
        i = if stuck { at + (end - i) } else { at };
    }
}

#[cfg(test)]
mod tests;
