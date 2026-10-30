use super::item::{Item, Rect, invert, ndc, scale_of};
use super::paint::{Painter, Target};
use super::pipes::Mode;
use super::{Op, Src};
use std::ops::Range;
use wprs_scene::{Affine, EffectDraw, Frame, Mask};

pub(crate) struct Fed<'e> {
    pub(crate) mask: Option<Src>,
    pub(crate) textures: [Option<(&'e str, Src)>; TEXTURES],
}

const TEXTURES: usize = 24;

impl Fed<'_> {
    pub(crate) fn get(&self, param: &str) -> Option<Src> {
        self.textures.iter().flatten().find(|(p, _)| *p == param).map(|(_, s)| *s)
    }

    pub(crate) fn all(&self) -> impl Iterator<Item = Src> + '_ {
        self.mask.into_iter().chain(self.textures.iter().flatten().map(|(_, s)| *s))
    }
}

fn bound<'e>(e: &EffectDraw<'e>) -> impl Iterator<Item = (&'e str, &'e str)> + use<'e> {
    let params = e.effect.params.iter();
    params.filter_map(|(p, v)| Some((p.as_str(), v.binding()?.layer.as_deref()?)))
}

fn quad(color: [f32; 4]) -> Item {
    Item { quad: [0.0, 0.0, 1.0, 1.0], uv: [0.0, 0.0, 1.0, 1.0], color, ..Item::default() }
}

pub(crate) fn feeds<'a>(frame: &'a Frame<'_>, range: Range<usize>) -> impl Iterator<Item = usize> + 'a {
    let draws = &frame.draws[range];
    let effects = draws.iter().flat_map(move |d| &frame.effects[d.effects.clone()]);
    let ids = effects.flat_map(|e| bound(e).map(|(_, id)| id).chain(e.mask.and_then(|m| m.layer.as_deref())));
    let masks = draws.iter().filter_map(|d| d.mask.and_then(|m| m.layer.as_deref()));
    ids.chain(masks).filter_map(move |id| frame.draws.iter().position(|d| d.layer.id == id))
}

impl Painter<'_, '_, '_> {
    pub(crate) fn feeds(&self, range: Range<usize>) -> impl Iterator<Item = usize> + use<'_> {
        feeds(self.frame, range)
    }

    pub(crate) fn feeding(&mut self, range: Range<usize>) -> (u64, bool) {
        let frame = self.frame;
        feeds(frame, range).fold((0, false), |(h, moving), j| {
            let (key, moves) = self.upstream(j);
            (h.rotate_left(7) ^ key, moving || moves)
        })
    }

    fn upstream(&mut self, j: usize) -> (u64, bool) {
        match self.chains.get(j) {
            Some(Some(found)) => return *found,
            Some(None) => self.chains[j] = Some((0, true)),
            None => return (0, true),
        }
        let frame = self.frame;
        let end = frame.draws[j].end;
        let (fed, moving) = self.feeding(j..end);
        let video = |d: &wprs_scene::Draw<'_>| matches!(d.content, wprs_scene::Content::Video { .. });
        let moving = moving || frame.draws[j..end].iter().any(|d| d.dynamic || video(d));
        let found = (self.digest(j..end, (fed, if moving { self.moment } else { 0 })), moving);
        self.chains[j] = Some(found);
        found
    }

    pub(crate) fn shared(&mut self, j: usize) -> Option<(Src, Rect)> {
        if let Some((_, found)) = self.isolated.iter().find(|(k, _)| *k == j) {
            return *found;
        }
        self.isolated.push((j, None));
        let found = self.isolate(j);
        if let Some(slot) = self.isolated.iter_mut().find(|(k, _)| *k == j) {
            slot.1 = found;
        }
        found
    }

    pub(crate) fn unshare(&mut self) {
        for (_, found) in self.isolated.drain(..) {
            if let Some((Src::Pool(i), _)) = found {
                self.pool.release(i);
            }
        }
    }

    pub(crate) fn fed<'e>(&mut self, i: usize, e: &EffectDraw<'e>, rect: Rect, uv: Affine) -> Fed<'e> {
        let mut fed = Fed { mask: None, textures: [None; TEXTURES] };
        let mask = e.mask.filter(|m| m.layer.is_some() || m.shape.is_some());
        if mask.is_none() && bound(e).next().is_none() {
            return fed;
        }
        let m = rect.affine().then(invert(uv));
        let cap = self.full.w.max(self.full.h).max(1.0);
        let side = |a: f32, b: f32| a.hypot(b).ceil().clamp(1.0, cap) as u32;
        let size = (side(m.0[0], m.0[1]), side(m.0[2], m.0[3]));
        for ((param, id), slot) in bound(e).zip(&mut fed.textures) {
            *slot = self.drawn(id, m, size).map(|src| (param, src));
        }
        fed.mask = mask.and_then(|mask| match &mask.layer {
            Some(id) => self.drawn(id, m, size),
            None => self.outline(i, mask, size),
        });
        fed
    }

    fn drawn(&mut self, id: &str, m: Affine, (w, h): (u32, u32)) -> Option<Src> {
        let j = self.frame.draws.iter().position(|d| d.layer.id == id)?;
        let (src, r) = self.shared(j)?;
        let out = self.acquire((w, h));
        if let Some(out) = out {
            let full = Rect { x: 0.0, y: 0.0, w: w as f32, h: h as f32 };
            let mut item = quad([1.0; 4]);
            (item.m0, item.m1) = ndc(full, full.affine().then(invert(m)).then(r.affine()));
            self.paint(out, item, [src, Src::White, Src::Clear]);
        }
        out
    }

    fn outline(&mut self, i: usize, mask: &Mask, (w, h): (u32, u32)) -> Option<Src> {
        let out = self.acquire((w, h))?;
        let full = Rect { x: 0.0, y: 0.0, w: w as f32, h: h as f32 };
        let scale = scale_of(self.screen).max(1e-6);
        let size = self.geometry(i).map_or([w as f32 / scale, h as f32 / scale], |(size, _)| size);
        let mut item = quad([1.0; 4]);
        (item.m0, item.m1) = ndc(full, full.affine());
        let target = Target { src: out, rect: full };
        let masked = self.mask(&mut item, (Some(mask), None, 1.0), full.affine(), size, target);
        item.mask_info[1] = 0.0;
        self.paint(out, item, [Src::White, masked, Src::Clear]);
        Some(out)
    }

    fn paint(&mut self, target: Src, item: Item, tex: [Src; 3]) {
        self.ops.push(Op::Clear(target, [0.0; 4]));
        let slot = self.pool.push(&item);
        self.ops.push(Op::Draw { target, entry: "fs_texture", mode: Mode::Replace, slot, tex });
    }
}
