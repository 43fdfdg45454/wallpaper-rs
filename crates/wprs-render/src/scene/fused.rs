use super::Src;
use super::effects::EMPTY;
use super::item::{Rect, channel, scale_of};
use super::paint::Painter;
use super::pipes::Shader;
use wprs_scene::{Affine, CHAIN_ENTRY, EffectDraw, Fuse, MASK_SLOTS};

impl Painter<'_, '_, '_> {
    pub(crate) fn chain(
        &mut self,
        i: usize,
        list: &[EffectDraw<'_>],
        input: Src,
        (rect, uv): (Rect, Affine),
    ) -> Src {
        let key =
            self.r.pipes().chain(&self.r.device, list.iter().map(|e| e.descriptor), Some(&mut *self.lease));
        let mut fx = super::placed::uniforms(&list[0], uv);
        let (mut tex, mut fed, mut base, mut first) = (EMPTY, [Src::Void; MASK_SLOTS], 0, None);
        tex[0] = input;
        for e in list {
            let n = e.descriptor.params.len();
            fx.slots[base..base + n].copy_from_slice(&e.slots[..n]);
            let mut pick = [0.0, 1.0, 0.0, 0.0];
            if let Some(m) = e.mask {
                let (src, crop) = match m.layer {
                    Some(_) => {
                        let fed = self.fed(i, e, (rect, uv), self.mask_reach(e, list));
                        (fed.mask, fed.crop)
                    }
                    None => {
                        let image = e.mask_texture.as_deref().and_then(|id| self.picture(id));
                        let crop = image.map_or([0.0; 4], |i| [0.0, 0.0, 1.0 / i.crop[0], 1.0 / i.crop[1]]);
                        (image.map(|image| self.ext(image)), crop)
                    }
                };
                let src = src.unwrap_or(Src::White);
                if m.layer.is_some()
                    && !fed.contains(&src)
                    && let Some(free) = fed.iter_mut().find(|f| **f == Src::Void)
                {
                    *free = src;
                }
                let name = (m.layer.is_some(), m.layer.as_deref().or(e.mask_texture.as_deref()));
                let slot = if *first.get_or_insert(name) == name { 1 } else { 5 };
                tex[slot] = src;
                fx.crop[usize::from(slot == 5)] = crop;
                fx.slots[base + n] = channel(m.channel, m.texture.is_some());
                pick = [
                    f32::from(u8::from(m.invert)),
                    1.0 - e.mask_strength,
                    f32::from(u8::from(slot == 5)),
                    0.0,
                ];
            }
            fx.slots[base + n + 1] = pick;
            base += n + MASK_SLOTS;
        }
        let out = self.pass(Shader::Fx(key, CHAIN_ENTRY), rect, None, 1.0, tex, fx);
        fed.into_iter().filter(|s| ![Src::Void, out].contains(s)).for_each(|s| self.release(s));
        out
    }

    fn mask_reach(&self, e: &EffectDraw<'_>, list: &[EffectDraw<'_>]) -> Option<f32> {
        let moved = list.iter().filter(|l| l.descriptor.fuse == Some(Fuse::Uv));
        let reach = moved.map(|l| l.descriptor.reach.as_ref().map(|_| l.reach)).sum::<Option<f32>>()?;
        e.descriptor.reads_mask_at_its_pixel().then_some(reach * scale_of(self.screen))
    }
}
