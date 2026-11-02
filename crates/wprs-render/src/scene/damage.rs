use super::item::Rect;
use super::paint::Painter;
use std::ops::Range;
use wprs_scene::{Blend, Color, Content, Fuse, Layer};

const MOVING: u64 = u64::MAX;
const MOST: f32 = 0.5;

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Reach {
    Own,
    Around,
    Matte,
}

pub(crate) type Mark = (usize, u64, Option<Rect>, Reach);

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Damage {
    Full,
    Part(Rect),
    Clean,
}

impl Painter<'_, '_, '_> {
    pub(crate) fn plain(&self, i: usize) -> bool {
        let d = &self.frame.draws[i];
        let plain = matches!(d.content, Content::Group { clip: false }) && d.opacity >= 1.0;
        let plain = plain && d.blend == Blend::Normal && d.mask.is_none() && d.effects.is_empty();
        let tilted = d.layer.transform.tilt.is_some() || d.layer.transform.z.is_some();
        plain && d.tint == Color([1.0; 4]) && d.brightness == 1.0 && !d.layer.cache && !tilted
    }

    fn marks(&mut self, range: Range<usize>, out: &mut Vec<Mark>) {
        let mut i = range.start;
        while i < range.end {
            let frame = self.frame;
            let d = &frame.draws[i];
            if self.plain(i) && !d.hidden {
                self.marks(i + 1..d.end, out);
            } else {
                let pointwise = |e: &wprs_scene::EffectDraw<'_>| {
                    e.descriptor.fuse == Some(Fuse::Color) && e.descriptor.reach.is_none()
                };
                let effects = &self.frame.effects[d.effects.clone()];
                let reach = match &d.content {
                    _ if d.mask.is_some_and(|m| m.layer.is_some()) => Reach::Matte,
                    _ if effects.iter().any(|e| e.descriptor.below()) => Reach::Matte,
                    _ if self.feeds(i..d.end).next().is_some() => Reach::Matte,
                    Content::Adjustment if self.captures(i).is_some() => Reach::Matte,
                    Content::Adjustment if !effects.iter().all(pointwise) => Reach::Around,
                    _ => Reach::Own,
                };
                let video = self.frame.draws[i..d.end].iter().any(|d| match &d.content {
                    Content::Video { .. } => true,
                    Content::Mesh { texture: Some(t), .. } => self.videos.iter().any(|(n, _)| n == t),
                    _ => false,
                });
                let (fed, moving) = self.feeding(i..d.end);
                let (ticking, flocks) = self.ticking(i..d.end);
                let moment = if ticking || moving { self.moment } else { 0 };
                let key = if video {
                    MOVING
                } else {
                    self.sealed(i..d.end, (d.hidden, moment, fed, flocks), false)
                };
                let rect = if d.hidden { None } else { self.extent(i) };
                out.push((std::ptr::from_ref::<Layer>(d.layer) as usize, key, rect, reach));
            }
            i = d.end;
        }
    }

    fn ticking(&self, range: Range<usize>) -> (bool, u64) {
        let plain = |d: &wprs_scene::Draw<'_>| d.effects.is_empty();
        self.frame.draws[range].iter().fold((false, 0), |(ticking, flocks), d| match d.content {
            Content::Particles { flock, .. } if plain(d) => {
                let stamp = self.flocks.get(flock).copied().unwrap_or(u64::MAX);
                (ticking, flocks.rotate_left(9) ^ stamp)
            }
            Content::Group { .. } if plain(d) => (ticking, flocks),
            _ => (ticking || d.dynamic, flocks),
        })
    }

    pub(crate) fn damage(&mut self, old: &[Mark], new: &mut Vec<Mark>, fresh: bool) -> Damage {
        new.clear();
        self.marks(0..self.frame.draws.len(), new);
        if self.frame.draws.iter().any(|d| d.lit.is_some() && !d.hidden) {
            let glow = &self.pool.glow;
            new.extend(
                glow.marks[..glow.count()].iter().map(|&(id, key, rect)| (id, key, Some(rect), Reach::Own)),
            );
        }
        if fresh || old.is_empty() {
            return Damage::Full;
        }
        let mut found: Option<Rect> = None;
        let full = self.full;
        let add = |found: &mut Option<Rect>, r: Option<Rect>| {
            if let Some(r) = r.and_then(|r| r.pixels(full)) {
                *found = Some(found.map_or(r, |f| f.union(r)));
            }
        };
        for &(id, key, rect, reach) in new.iter() {
            match old.iter().find(|m| m.0 == id) {
                Some(&(_, k, r, _)) if k == key && k != MOVING && r == rect => {
                    let near = |f: Rect| rect.is_some_and(|r| r.meets(f));
                    if reach == Reach::Around && found.is_some_and(near) {
                        add(&mut found, rect);
                    }
                }
                Some(&(.., r, _)) => {
                    add(&mut found, r);
                    add(&mut found, rect);
                }
                None => add(&mut found, rect),
            }
        }
        old.iter().filter(|m| !new.iter().any(|n| n.0 == m.0)).for_each(|m| add(&mut found, m.2));
        let mattes = new.iter().any(|m| m.3 == Reach::Matte);
        match found.and_then(|r| r.pixels(self.full)) {
            None => Damage::Clean,
            Some(_) if mattes => Damage::Full,
            Some(r) if r.w * r.h > MOST * self.full.w * self.full.h => Damage::Full,
            Some(r) => Damage::Part(r),
        }
    }

    pub(crate) fn skip(&mut self, i: usize, target: super::paint::Target) -> bool {
        let Some(clip) = self.clip.filter(|_| target.src == super::Src::Canvas) else { return false };
        let d = &self.frame.draws[i];
        if self.extent(i).is_some_and(|r| r.meets(clip)) {
            return false;
        }
        let ids = self.frame.draws[i..d.end].iter().map(|d| std::ptr::from_ref::<Layer>(d.layer) as usize);
        self.kept.extend(ids);
        true
    }
}

impl Painter<'_, '_, '_> {
    pub(crate) fn fill(&mut self, target: super::Src, color: [f64; 4]) {
        let full = [0.0, 0.0, 1.0, 1.0];
        let color = color.map(|c| c as f32);
        let mut item = super::item::Item { quad: full, uv: full, color, ..Default::default() };
        (item.m0, item.m1) = super::item::ndc(self.full, self.full.affine());
        let slot = self.pool.push(&item);
        let (entry, mode, tex) = (
            "fs_texture",
            super::pipes::Mode::Replace,
            [super::Src::White, super::Src::White, super::Src::Clear],
        );
        self.ops.push(super::Op::Draw { target, entry, mode, slot, tex });
    }
}
