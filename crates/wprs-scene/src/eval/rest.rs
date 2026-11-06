use crate::{Descriptor, Layer, PointerState};

pub(super) fn chain(
    layer: &Layer,
    i: usize,
    (d, slots): (&Descriptor, &[[f32; 4]]),
    pointer: &PointerState,
) -> Option<usize> {
    let chain = crate::chain::runs(&layer.effects).find(|r| r.start == i).map_or(1, |r| r.len());
    (!resting(d, slots, pointer, chain)).then_some(chain)
}

fn resting(d: &Descriptor, slots: &[[f32; 4]], pointer: &PointerState, chain: usize) -> bool {
    let builtin = |id: &str| crate::descriptor(id).is_some_and(|b| std::ptr::eq(b, d));
    let at = |id: &str| d.param(id).map_or(0.0, |i| slots[i][0]);
    if chain != 1 {
        return false;
    }
    if builtin("bloom") {
        return at("intensity") == 0.0;
    }
    if !builtin("depth_parallax") {
        return false;
    }
    let p = pointer.presence.clamp(0.0, 1.0);
    let follow = at("follow") * p * p * (3.0 - 2.0 * p);
    let aim = pointer.smooth.map(|c| (c - 0.5) * 2.0 * follow);
    let drift = aim[0].hypot(aim[1]).min(1.0) * at("strength").abs();
    at("overscan") <= 1.0 && at("orbit") == 0.0 && drift < 0.05
}
