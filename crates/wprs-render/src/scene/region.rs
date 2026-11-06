use super::inked::Inked;
use super::item::{channel, invert, scale_of};
use super::paint::Painter;
use super::{Fill, Op, Src};
use wprs_scene::{Affine, EffectDraw, MaskMode, Wrapping};

const MOST: f32 = 0.7;
const TILED_LEAST: f32 = 1_000_000.0;

pub(crate) fn worth(saved: f32, units: f32, tiled: bool) -> bool {
    !tiled || saved * units.max(1.0) >= TILED_LEAST
}

fn span(boxes: impl Iterator<Item = [f32; 4]>) -> Option<[f32; 4]> {
    let full = boxes.filter(|b| b[2] > b[0] && b[3] > b[1]);
    full.reduce(|a, b| [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])])
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Region {
    Whole,
    Empty,
    Part([u32; 4]),
}

impl Painter<'_, '_, '_> {
    fn masked(&self, e: &EffectDraw<'_>) -> Option<Option<[f32; 4]>> {
        let m = e.mask.filter(|m| m.layer.is_none() && m.shape.is_none() && !m.invert && m.feather <= 0.0)?;
        if e.descriptor.mask_mode != MaskMode::Strength || e.mask_strength < 1.0 {
            return None;
        }
        let inked = self.inked(e.mask_texture.as_deref()?)?;
        Some(span(channel(m.channel, true).into_iter().zip(inked).filter(|(w, _)| *w != 0.0).map(|(_, b)| b)))
    }

    fn bounded(&self, e: &EffectDraw<'_>) -> Option<Option<[f32; 4]>> {
        let d = e.descriptor;
        let named = d.bounded.iter().map(|b| d.params.get(d.param(b)?)?.named(e.effect));
        let all: Option<Vec<Inked>> = named.map(|id| self.inked(id?)).collect();
        all.filter(|a| !a.is_empty()).map(|a| span(a.into_iter().flat_map(|i| [i[0], i[1], i[2]])))
    }

    fn inked(&self, id: &str) -> Option<Inked> {
        self.samplings.get(id).filter(|s| s.wrap == Wrapping::Clamp)?.inked
    }

    pub(crate) fn region(&self, e: &EffectDraw<'_>, uv: Affine, (w, h): (u32, u32)) -> Region {
        let d = e.descriptor;
        let single = d.passes.len() == 1 && d.state.is_none() && d.targets.is_empty() && !d.below();
        if !single || e.chain > 1 || (d.reach.is_none() && !d.reads_its_pixel()) {
            return Region::Whole;
        }
        let found = match (self.masked(e), self.bounded(e)) {
            (None, None) => return Region::Whole,
            (Some(a), None) | (None, Some(a)) => a,
            (Some(a), Some(b)) => {
                a.zip(b).map(|(a, b)| [a[0].max(b[0]), a[1].max(b[1]), a[2].min(b[2]), a[3].min(b[3])])
            }
        };
        let Some([x0, y0, x1, y1]) = found.filter(|b| b[2] > b[0] && b[3] > b[1]) else {
            return Region::Empty;
        };
        let back = invert(uv);
        let corners = [[x0, y0], [x1, y0], [x0, y1], [x1, y1]].map(|p| back.apply(p));
        let (lo, hi) = corners.iter().fold(([f32::MAX; 2], [f32::MIN; 2]), |(lo, hi), p| {
            ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])])
        });
        let grow = e.reach * scale_of(self.screen) * self.density + 2.0;
        let side = |v: f32, n: u32| (v * n as f32).clamp(0.0, n as f32);
        let (a, b) = ((side(lo[0], w) - grow).max(0.0).floor(), (side(lo[1], h) - grow).max(0.0).floor());
        let (c, d) =
            ((side(hi[0], w) + grow).min(w as f32).ceil(), (side(hi[1], h) + grow).min(h as f32).ceil());
        if c <= a || d <= b {
            return Region::Empty;
        }
        let (all, part) = ((w * h) as f32, (c - a) * (d - b));
        if part > MOST * all || !worth(all - part, e.descriptor.units(), self.r.tiled) {
            return Region::Whole;
        }
        Region::Part([a as u32, b as u32, (c - a) as u32, (d - b) as u32])
    }

    pub(crate) fn in_place(
        &mut self,
        from: usize,
        (input, current, next): (Src, Src, Src),
        [x, y, w, h]: [u32; 4],
    ) -> Option<Src> {
        let size = self.size_of(current);
        if current == input || !matches!(current, Src::Pool(_)) || size != self.size_of(next) {
            return None;
        }
        let mut drawn = self.ops[from..]
            .iter()
            .enumerate()
            .filter(|(_, op)| matches!(op, Op::Effect(f) if f.target == next));
        let (at, _) = drawn.next().filter(|_| drawn.next().is_none())?;
        let at = from + at;
        let Op::Effect(f) = self.ops[at] else { return None };
        let copy = self.acquire(size)?;
        let mut tex = self.sets[f.tex as usize];
        tex.iter_mut().filter(|t| **t == current).for_each(|t| *t = copy);
        let tex = self.set(tex);
        self.ops[at] =
            Op::Effect(Fill { target: current, load: true, scissor: Some([x, y, w, h]), tex, ..f });
        self.ops.insert(at, Op::Copy { from: current, to: copy, origin: [x, y], size: (w, h), at: [x, y] });
        self.release(next);
        self.release(copy);
        Some(current)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn tiled_gpus_narrow_an_effect_only_when_the_work_saved_outweighs_a_copy_and_a_reload() {
        assert!(super::worth(1000.0, 1.0, false));
        assert!(!super::worth(800_000.0, 1.0, true));
        assert!(super::worth(800_000.0, 2.0, true) && super::worth(2_000_000.0, 1.0, true));
    }
}
