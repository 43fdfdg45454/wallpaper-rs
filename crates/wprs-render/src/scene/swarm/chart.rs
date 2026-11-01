use super::maps::{Asked, SIDE};
use crate::scene::Op;
use crate::scene::paint::Painter;
use wprs_scene::Affine;

impl Painter<'_, '_, '_> {
    pub(crate) fn chart(&mut self, flock: usize) {
        let (frame, n) = (self.frame, self.swarms.as_ref().map_or(0, |s| s.maps.len()));
        for i in 0..n {
            let Some(m) = self.swarms.as_deref().and_then(|s| s.maps.get(i)).filter(|m| m.herd == flock)
            else {
                continue;
            };
            let Some(j) = frame.draws.iter().position(|d| d.layer.id == m.source) else { continue };
            let key = self.upstream(j).0;
            let wanted = self.swarms.as_deref().is_some_and(|s| s.due(i, key, frame.time));
            let Some((_, rect)) = wanted.then(|| self.shared(j)).flatten() else { continue };
            let longest = rect.w.max(rect.h).max(1.0);
            let side = (if longest > 1024.0 { SIDE } else { SIDE / 2 }) as f32;
            let scale = side.min(longest.ceil()) / longest;
            let size = ((rect.w * scale).ceil().max(1.0) as u32, (rect.h * scale).ceil().max(1.0) as u32);
            let id = frame.draws[j].layer.id.as_str();
            let Some(out) = self.drawn(id, rect.affine(), size) else { continue };
            self.ops.push(Op::Read { from: out, map: i, size });
            self.release(out);
            let place = frame.draws[j].world.inverse().then(self.screen.inverse()).then(rect.affine());
            let place = place.then(Affine::scale([1.0 / size.0 as f32, 1.0 / size.1 as f32]));
            let format = self.format;
            if let Some(m) = self.swarms.as_deref_mut().and_then(|s| s.maps.get_mut(i)) {
                (m.asked, m.sent, m.key, m.at) =
                    (Some(Asked { size, place, format }), true, Some(key), frame.time);
            }
        }
    }
}
