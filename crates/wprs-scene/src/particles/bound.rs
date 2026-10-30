use super::{EmitShape, MAX_EMITTERS2, Sim, Spread, System};
use crate::resolve::Context;

pub(super) fn bound(sim: &mut Sim, s: &System, cx: &Context) {
    let i = &s.init;
    let spans = [
        (Some(&i.life), [0, 0], [1e-3, 600.0]),
        (Some(&i.speed), [0, 2], [0.0, 1e5]),
        (Some(&i.angle), [1, 0], [-3600.0, 3600.0]),
        (Some(&i.size), [1, 2], [0.0, 4096.0]),
        (Some(&i.rotation), [2, 0], [-3600.0, 3600.0]),
        (Some(&i.spin), [2, 2], [-1e5, 1e5]),
        (Some(&i.frame), [3, 0], [0.0, 4096.0]),
        (i.alpha.as_ref(), [3, 2], [0.0, 1.0]),
        (i.radial.as_ref(), [4, 0], [-1e5, 1e5]),
    ];
    for (spread, [row, col], [min, max]) in spans {
        let Some(ends) = spread.and_then(|s: &Spread<f32>| s.bound.as_deref()) else { continue };
        let lo = ends[0].get(cx).clamp(min, max);
        (sim.init[row][col], sim.init[row][col + 1]) = (lo, ends[1].get(cx).clamp(lo, max));
    }
    for (e, em) in s.emitters.iter().take(MAX_EMITTERS2).enumerate() {
        let EmitShape::Circle { radius, inner, .. } = &em.shape else { continue };
        let slot = &mut sim.emitters[4 * e];
        if let Some(b) = &radius.1 {
            slot[2] = b.eval(cx).clamp(0.0, 1e5);
        }
        if let Some(b) = &inner.1 {
            slot[3] = b.eval(cx).clamp(0.0, slot[2]);
        }
    }
}
