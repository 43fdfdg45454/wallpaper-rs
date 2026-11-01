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
        let num = |n: &super::Num, lo: f32, hi: f32| n.1.as_ref().map(|b| b.eval(cx).clamp(lo, hi));
        let [a, b] = &mut sim.emitters[4 * e..4 * e + 2] else { continue };
        match &em.shape {
            EmitShape::Circle { radius, inner, .. } => {
                num(radius, 0.0, 1e5).into_iter().for_each(|r| a[2] = r);
                num(inner, 0.0, a[2]).into_iter().for_each(|r| a[3] = r);
            }
            EmitShape::Around(r) => {
                num(&r.radius, 0.0, 1e5).into_iter().for_each(|v| a[2] = v);
                num(&r.arc, 0.0, 360.0).into_iter().for_each(|v| a[3] = v);
                num(&r.speed, -1e4, 1e4).into_iter().for_each(|v| b[3] = v);
            }
            EmitShape::Between(w) => num(&w.span, 0.0, 1.0).into_iter().for_each(|v| b[1] = v),
            EmitShape::Layer(l) => num(&l.threshold, 0.0, 1.0).into_iter().for_each(|v| b[0] = v),
            _ => {}
        }
    }
    for (row, d) in [(0, &i.depth), (1, &i.depth_speed)] {
        let Some(ends) = d.as_ref().and_then(|s: &Spread<f32>| s.bound.as_deref()) else { continue };
        let lo = ends[0].get(cx).clamp(-1e5, 1e5);
        (sim.depth[row][0], sim.depth[row][1]) = (lo, ends[1].get(cx).clamp(lo, 1e5));
    }
}
