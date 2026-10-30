use super::tune::moved;
use super::{MAX_CONTROL, Placement, System, Target};
use crate::resolve::Context;
use crate::{Affine, Layer};

#[derive(Debug, Clone, Copy)]
pub(super) struct Room {
    pub frame: Affine,
    pub cursor: [f32; 3],
    pub points: [[f32; 3]; MAX_CONTROL],
}

pub(super) fn turn(a: Affine) -> f32 {
    a.0[1].atan2(a.0[0]).to_degrees()
}

pub(super) fn bare(a: Affine) -> Affine {
    let [a, b, c, d, ..] = a.0;
    Affine([a, b, c, d, 0.0, 0.0])
}

impl Room {
    pub(super) fn new(
        cx: &Context,
        s: &System,
        (frame, to_sim): (Affine, Affine),
        (cursor, home): ([f32; 3], [f32; 2]),
        layer: Option<&Layer>,
    ) -> Room {
        let mut points = [[0.0; 3]; MAX_CONTROL];
        for (k, out) in points.iter_mut().enumerate() {
            let c = s.points.get(k);
            let angle = c.and_then(|c| c.angle.as_ref()).map_or(0.0, |a| a.get(cx)) + turn(frame);
            let at = if c.is_some_and(|c| c.follow.is_some()) {
                if cursor[2] > 0.5 { [cursor[0], cursor[1]] } else { home }
            } else {
                let at =
                    moved(cx, layer, s, k).or_else(|| c.and_then(|c| c.at.as_ref()).map(|v| v.get(cx).0));
                let at = at.unwrap_or_default();
                match c.is_some_and(|c| c.space == Placement::World) {
                    true => to_sim.apply(at),
                    false => frame.apply(at),
                }
            };
            *out = [at[0], at[1], angle];
        }
        Room { frame, cursor, points }
    }

    pub(super) fn aim(&self, t: &Target) -> [f32; 3] {
        match t {
            Target::Point(p) => {
                let [x, y] = self.frame.apply(*p);
                [x, y, 1.0]
            }
            Target::Follow(_) => self.cursor,
            Target::Control(h) => {
                let [x, y, _] = self.points[usize::from(h.0).min(MAX_CONTROL - 1)];
                [x, y, 1.0]
            }
        }
    }
}
