use super::mode;
use crate::scene::Op;
use crate::scene::item::ndc;
use crate::scene::paint::{Painter, Target};
use wprs_scene::{Affine, Blend, Content, Mask};

impl Painter<'_, '_, '_> {
    pub(crate) fn particles(
        &mut self,
        i: usize,
        target: Target,
        (opacity, blend, mask): (f32, Blend, Option<&Mask>),
        atop: bool,
    ) {
        let frame = self.frame;
        let d = &frame.draws[i];
        let Content::Particles { flock, size, .. } = d.content else { return };
        self.chart(flock);
        if mask.is_some() || !blend.fixed() || self.lean(i).is_some() {
            return self.apart(i, target, atop);
        }
        let (r, screen, srgb) = (self.r, self.screen, self.srgb);
        let Some(swarms) = self.swarms.as_deref_mut() else { return };
        let mode = mode(blend, atop);
        swarms.pipeline(r, mode);
        let Some(herd) = swarms.herds.get(flock) else { return };
        let [w, h] = swarms.canvas;
        let m = match herd.world {
            true => screen.then(frame.camera),
            false => screen.then(d.world).then(Affine::translate([-d.origin[0] * w, -d.origin[1] * h])),
        };
        let tinted = d.tint != wprs_scene::Color([1.0; 4]) || d.brightness != 1.0;
        let [cr, cg, cb, _] = d.tint.0;
        let toned = crate::scene::item::toned(wprs_scene::Color([cr, cg, cb, 1.0]), d.brightness, srgb, 1.0);
        let tint = [toned[0], toned[1], toned[2], d.tint.0[3]];
        let slot = (swarms.looks.len() as u64 * super::LOOK) as u32;
        let mut timed = 0;
        for (p, system) in herd.bake.systems.iter().enumerate() {
            let own = (p == 0).then_some(d.layer.id.as_str());
            let found = |layer: Option<&str>| {
                swarms.kits.iter().position(|k| k.system == *system && k.layer.as_deref() == layer)
            };
            let Some(k) = own.and_then(|l| found(Some(l))).or_else(|| found(None)) else { continue };
            let staged = swarms.kits[k].stage.as_ref().map(|s| s.0);
            if let Some((key, why)) = staged.and_then(|key| Some((key, super::staging::left(key)?))) {
                super::stage::leave(&mut swarms.kits, key, &why);
            }
            if let Some(key) = swarms.kits[k].stage.as_ref().map(|s| s.0) {
                self.drawn.push(key);
                timed = if timed == 0 { key } else { timed };
            }
            let kit = &swarms.kits[k];
            if let Some((id, why)) = &kit.refused {
                self.refused.entry(id.clone()).or_insert_with(|| why.clone());
            }
            let mut look = kit.look;
            if tinted {
                look.colors.iter_mut().for_each(|c| (0..4).for_each(|j| c[j] *= tint[j]));
            }
            (look.m0, look.m1) = ndc(target.rect, m);
            look.turn[2] = m.0[1].atan2(m.0[0]);
            let middle = m.inverse().apply(screen.apply([w / 2.0, h / 2.0]));
            (look.deep[1], look.deep[2]) = (middle[0], middle[1]);
            look.tint[0] = opacity;
            look.tint[2] = herd.bake.sizes[p] * size;
            let trail = herd.bake.trails[p];
            let q = swarms.quality;
            let drawn =
                if kit.long { q.long_trail(kit.trail.min(trail)) } else { q.trail(kit.trail.min(trail)) };
            (look.extra[1], look.extra[2]) = (drawn as f32, trail as f32);
            look.strand[0] = q.subdivide(look.strand[0] as u32) as f32;
            let scroll = |v: f32| (f64::from(v) * f64::from(frame.time)).rem_euclid(1.0) as f32;
            (look.strand[2], look.cord[2]) = (scroll(look.strand[2]), scroll(look.cord[2]));
            swarms.looks.push(look);
            swarms.kitted.push(k);
        }
        self.ops.push(Op::Particles { target: target.src, system: flock, slot, mode, timed });
    }
}
