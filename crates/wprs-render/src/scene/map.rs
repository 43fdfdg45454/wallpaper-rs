use super::item::{Item, Rect, invert, ndc, rows};
use super::paint::Painter;
use wprs_scene::{Affine, Lean};

#[derive(Debug, Clone, Copy)]
pub(crate) enum Map {
    Flat(Affine),
    Lean(Lean),
}

fn split(l: Lean) -> ([f32; 4], [f32; 4], [f32; 4]) {
    let m = l.0;
    ([m[0], m[1], m[2], 0.0], [m[3], m[4], m[5], 0.0], [m[6], m[7], m[8] - 1.0, 0.0])
}

impl Map {
    pub(crate) fn then(self, a: Affine) -> Map {
        match self {
            Map::Flat(m) => Map::Flat(m.then(a)),
            Map::Lean(l) => Map::Lean(l.then(Lean::of(a))),
        }
    }

    pub(crate) fn after(self, a: Affine) -> Map {
        match self {
            Map::Flat(m) => Map::Flat(a.then(m)),
            Map::Lean(l) => Map::Lean(Lean::of(a).then(l)),
        }
    }

    pub(crate) fn inverse(self) -> Map {
        match self {
            Map::Flat(m) => Map::Flat(invert(m)),
            Map::Lean(l) => Map::Lean(l.inverse()),
        }
    }

    pub(crate) fn apply(self, p: [f32; 2]) -> [f32; 2] {
        match self {
            Map::Flat(m) => m.apply(p),
            Map::Lean(l) => l.apply(p),
        }
    }

    pub(crate) fn bounds(self, [x0, y0, x1, y1]: [f32; 4]) -> Option<Rect> {
        Rect::around([[x0, y0], [x1, y0], [x0, y1], [x1, y1]].map(|p| self.apply(p)))
    }

    pub(crate) fn place(self, item: &mut Item, target: Rect) {
        match self {
            Map::Flat(m) => (item.m0, item.m1) = ndc(target, m),
            Map::Lean(l) => {
                let to = ndc(target, Affine::IDENTITY);
                let to = Affine([to.0[0], to.1[0], to.0[1], to.1[1], to.0[2], to.1[2]]);
                (item.m0, item.m1, item.lean) = split(Lean::of(to).then(l));
            }
        }
    }

    pub(crate) fn sample(self, item: &mut Item) {
        match self {
            Map::Flat(m) => (item.mask_m0, item.mask_m1) = rows(m),
            Map::Lean(l) => (item.mask_m0, item.mask_m1, item.mask_lean) = split(l),
        }
    }
}

impl Painter<'_, '_, '_> {
    pub(crate) fn lean(&self, i: usize) -> Option<Lean> {
        let own = self.frame.draws[i].lean?;
        match self.flat.get() {
            Some((outer, _)) if outer == own => None,
            Some((_, undo)) => Some(undo.then(own)),
            None => Some(own),
        }
    }

    pub(crate) fn map(&self, i: usize, m: Affine) -> Map {
        let screen = Lean::of(self.screen);
        let pixels = |e: Lean| screen.then(e).then(Lean::of(invert(self.screen)));
        self.lean(i).map_or(Map::Flat(m), |e| Map::Lean(pixels(e).then(Lean::of(m))))
    }

    pub(crate) fn within<T>(&self, i: usize, f: impl FnOnce(&Self) -> T) -> T {
        let own = self.lean(i).and(self.frame.draws[i].lean).map(|l| (l, l.inverse()));
        let outer = own.map(|own| self.flat.replace(Some(own)));
        let out = f(self);
        outer.into_iter().for_each(|o| self.flat.set(o));
        out
    }
}
