use super::super::item::Rect;
use super::super::paint::Painter;
use super::Pool;
use crate::scene::Src;
use wgpu::TextureFormat;
use wprs_scene::{Affine, EffectDraw};

fn why(limit: u64, cap: u64) -> String {
    let what = if limit == cap { "this view may hold" } else { "a frame may borrow" };
    format!("its textures would take more than the {} MiB of textures {what}", limit >> 20)
}

impl Pool {
    pub(super) fn refuse(&mut self, limit: u64) -> Option<usize> {
        self.short = Some(limit);
        None
    }

    pub(super) fn unbind(&mut self, gone: &[u64]) {
        self.groups.retain(|key, _| !key[1..].iter().any(|id| *id != 0 && gone.contains(id)));
    }
}

impl Painter<'_, '_, '_> {
    pub(crate) fn borrow(&mut self, size: (u32, u32), format: TextureFormat) -> Option<usize> {
        loop {
            if let Some(found) = self.pool.acquire(&self.r.device, size, format) {
                return Some(found);
            }
            let (kept, held) = (&*self.kept, self.pool.short == Some(self.pool.cap));
            let spare = self.caches.iter().filter(|(id, _)| held && **id != 0 && !kept.contains(id));
            let id = spare.max_by_key(|(_, c)| c.tex.bytes()).map(|(id, _)| *id)?;
            let gone = self.caches.remove(&id)?.tex;
            self.pool.outside = self.pool.outside.saturating_sub(gone.bytes());
            self.pool.unbind(&[gone.id]);
        }
    }

    pub(crate) fn hold(&mut self, i: usize, size: (u32, u32)) -> Option<Src> {
        let found = self.acquire(size);
        if found.is_none() {
            self.missed(i, None);
        }
        found
    }

    pub(crate) fn missed(&mut self, i: usize, only: Option<&EffectDraw<'_>>) {
        let Some(limit) = self.pool.short.take() else { return };
        let frame = self.frame;
        let list = &frame.effects[frame.draws[i].effects.clone()];
        for e in only.map_or(list, std::slice::from_ref) {
            if !self.cut(i, e) {
                self.leave(i, e, why(limit, self.pool.cap));
            }
        }
    }

    pub(crate) fn step(
        &mut self,
        i: usize,
        list: &[EffectDraw<'_>],
        (input, current): (Src, Src),
        rect: Rect,
        uv: Affine,
    ) -> Src {
        self.pool.short = None;
        let next = match list {
            [e] if e.descriptor.key != 0 => self.own(i, e, input, current, rect, uv),
            [e] => self.effect(i, e, input, current, rect, uv),
            _ => self.chain(list, current, rect, uv),
        };
        self.missed(i, list.first());
        next
    }
}
