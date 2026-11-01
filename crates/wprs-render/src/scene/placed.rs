use super::item::{Fx, Rect, channel, rows, scale_of};
use super::paint::Painter;
use super::pipes::{Mode, Shader};
use super::{Fill, Op, Src};
use wprs_scene::{Affine, Descriptor, EffectDraw};

impl Painter<'_, '_, '_> {
    pub(crate) fn pass(
        &mut self,
        shader: Shader,
        rect: Rect,
        out: Option<Src>,
        scale: f32,
        tex: [Src; 7],
        fx: Fx,
    ) -> Src {
        let Some((target, slot)) = self.place(rect, out, scale, tex[0], fx) else { return tex[0] };
        let mut all = [Src::Clear; 11];
        all[..7].copy_from_slice(&tex);
        let (more, mode, load, vertices, tex) = (None, Mode::Replace, false, 3, self.set(all));
        self.ops.push(Op::Effect(Fill { target, shader, slot, tex, more, mode, load, vertices }));
        target
    }

    pub(crate) fn set(&mut self, tex: [Src; 11]) -> u32 {
        self.sets.push(tex);
        self.sets.len() as u32 - 1
    }

    pub(crate) fn bare(&mut self, entry: &'static str, rect: Rect, tex: [Src; 7], fx: Fx) -> Src {
        let lease = Some(&mut *self.lease);
        let plain = self.r.pipes().chain(&self.r.device, std::iter::empty::<&Descriptor>(), lease);
        self.pass(Shader::Fx(plain, entry), rect, None, 1.0, tex, fx)
    }

    pub(crate) fn place(
        &mut self,
        rect: Rect,
        out: Option<Src>,
        scale: f32,
        first: Src,
        fx: Fx,
    ) -> Option<(Src, u32)> {
        let size = ((rect.w * scale).ceil().max(1.0) as u32, (rect.h * scale).ceil().max(1.0) as u32);
        let out = out.or_else(|| self.acquire(size))?;
        let (w, h) = self.size_of(first);
        let px = scale_of(self.screen) * w as f32 / rect.w.max(1.0);
        let f = self.full;
        let (x0, y0) = (((f.x - rect.x) / rect.w).max(0.0), ((f.y - rect.y) / rect.h).max(0.0));
        let (x1, y1) = (((f.x + f.w - rect.x) / rect.w).min(1.0), ((f.y + f.h - rect.y) / rect.h).min(1.0));
        let fx = Fx {
            texel: [1.0 / w as f32, 1.0 / h as f32, w as f32, h as f32],
            info: [fx.info[0], fx.info[1], fx.info[2], px],
            area: [rect.x, rect.y, rect.w, rect.h],
            clip: [x0, y0, x1.max(x0), y1.max(y0)],
            ..fx
        };
        Some((out, self.pool.push(&fx)))
    }

    pub(crate) fn base(&mut self, e: &EffectDraw<'_>, layer_uv: Affine) -> (Fx, Src) {
        let images = self.images;
        let mask = e.mask_texture.as_deref().and_then(|id| images.get(id));
        let mask = mask.map_or(Src::White, |image| self.ext(&image.0));
        let (mut mask_m0, mask_m1) = rows(layer_uv);
        mask_m0[3] = 1.0 - e.mask_strength;
        let fx = Fx {
            slots: e.slots,
            texel: [0.0; 4],
            channel: e.mask.map_or([0.0, 0.0, 0.0, 1.0], |m| channel(m.channel, m.texture.is_some())),
            mask_m0,
            mask_m1,
            info: [f32::from(u8::from(e.mask.is_some_and(|m| m.invert))), 0.0, 0.0, 0.0],
            area: [0.0; 4],
            clip: [0.0, 0.0, 1.0, 1.0],
            velocity: [e.velocity[0], e.velocity[1], 0.0, 0.0],
        };
        (fx, mask)
    }
}
