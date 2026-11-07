use crate::{Descriptor, Layer, PointerState, Quality, Scene};

pub(super) fn chain(
    (scene, layer, quality): (&Scene, &Layer, Quality),
    i: usize,
    (d, slots): (&Descriptor, &[[f32; 4]]),
    pointer: &PointerState,
) -> Option<usize> {
    let cut = |e: &crate::EffectUse| crate::chain::cut_at(scene, e, quality);
    let found = crate::chain::runs_by(&layer.effects, cut).find(|r| r.start == i);
    let chain = found.map_or(1, |r| layer.effects[r].iter().filter(|e| !cut(e)).count());
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
