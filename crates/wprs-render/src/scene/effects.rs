use super::item::{Fx, Rect, channel, rows, scale_of};
use super::paint::Painter;
use super::pipes::Shader;
use super::{Op, Src};
use wprs_scene::{Affine, CHAIN_ENTRY, Descriptor, EffectDraw, MaskMode, Output, ParamKind};

const EMPTY: [Src; 7] = [Src::Clear, Src::White, Src::Clear, Src::Clear, Src::Clear, Src::Clear, Src::Void];

impl Painter<'_, '_, '_> {
    fn pass(
        &mut self,
        shader: Shader,
        rect: Rect,
        out: Option<Src>,
        scale: f32,
        tex: [Src; 7],
        fx: Fx,
    ) -> Src {
        let size = ((rect.w * scale).ceil().max(1.0) as u32, (rect.h * scale).ceil().max(1.0) as u32);
        let Some(out) = out.or_else(|| self.acquire(size)) else { return tex[0] };
        let (w, h) = self.size_of(tex[0]);
        let px = scale_of(self.screen) * w as f32 / rect.w.max(1.0);
        let fx = Fx {
            texel: [1.0 / w as f32, 1.0 / h as f32, w as f32, h as f32],
            info: [fx.info[0], fx.info[1], fx.info[2], px],
            area: [rect.x, rect.y, rect.w, rect.h],
            ..fx
        };
        let slot = self.pool.push(&fx);
        self.ops.push(Op::Effect { target: out, shader, slot, tex });
        out
    }

    fn base(&mut self, e: &EffectDraw<'_>, layer_uv: Affine) -> (Fx, Src) {
        let images = self.images;
        let mask = e.mask_texture.as_deref().and_then(|id| images.get(id));
        let mask = mask.map_or(Src::White, |(image, _)| self.ext(image));
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
        };
        (fx, mask)
    }

    pub(crate) fn effects(&mut self, i: usize, input: Src, rect: Rect, layer_uv: Affine) -> Src {
        let frame = self.frame;
        let list = &frame.effects[frame.draws[i].effects.clone()];
        let (mut current, mut k) = (input, 0);
        while k < list.len() {
            let n = list[k].chain.clamp(1, list.len() - k);
            current = match n {
                1 => self.effect(&list[k], input, current, rect, layer_uv),
                _ => self.chain(&list[k..k + n], current, rect, layer_uv),
            };
            k += n;
        }
        current
    }

    fn chain(&mut self, list: &[EffectDraw<'_>], input: Src, rect: Rect, layer_uv: Affine) -> Src {
        let key = self.r.pipes().chain(&self.r.device, list.iter().map(|e| e.descriptor), false);
        let (mut fx, _) = self.base(&list[0], layer_uv);
        let mut base = 0;
        for e in list {
            let n = e.descriptor.params.len();
            fx.slots[base..base + n].copy_from_slice(&e.slots[..n]);
            base += n;
        }
        let mut tex = EMPTY;
        tex[0] = input;
        self.pass(Shader::Fx(key, CHAIN_ENTRY), rect, None, 1.0, tex, fx)
    }

    fn effect(&mut self, e: &EffectDraw<'_>, source: Src, current: Src, rect: Rect, layer_uv: Affine) -> Src {
        let d: &'static Descriptor = e.descriptor;
        let key = self.r.pipes().chain(&self.r.device, std::iter::once(d), false);
        let (fx, mask) = self.base(e, layer_uv);
        let lut = self.lut(e);
        let tune = d.scale_at(self.frame.quality);
        let sim = d.state.map(|s| {
            let side = |v: f32| (v * s.scale * tune).ceil().max(1.0) as u32;
            let (id, size) = (std::ptr::from_ref(e.effect) as usize, (side(rect.w), side(rect.h)));
            self.sim(id, size);
            (id, size, self.ticks(id, self.frame.time))
        });
        let (mut outputs, mut previous, last) =
            (Vec::with_capacity(d.passes.len()), current, d.passes.len() - 1);
        for (n, pass) in d.passes.iter().enumerate() {
            let state = pass.output == Output::State;
            let runs = sim.filter(|_| state).map_or(1, |(.., ticks)| ticks);
            for k in 0..runs {
                let front = sim.map(|(id, size, _)| (id, self.sim(id, size)));
                let mut tex = [Src::Clear, mask, Src::Clear, Src::Clear, Src::Clear, Src::Clear, lut];
                for (at, name) in [0, 2, 3, 4, 5].into_iter().zip(&pass.inputs) {
                    tex[at] = match name.as_str() {
                        "previous" => previous,
                        "input" => current,
                        "source" => source,
                        "mask" => mask,
                        "state" => front.map_or(Src::Clear, |(id, side)| Src::State(id, side)),
                        other => match other.split_once(':') {
                            Some(("pass", k)) => k.parse().ok().and_then(|k: usize| outputs.get(k).copied()),
                            Some(("asset", p)) => self.asset(e, p, ParamKind::Texture),
                            _ => None,
                        }
                        .unwrap_or(Src::Clear),
                    };
                }
                let out = front.filter(|_| state).map(|(id, side)| Src::State(id, 1 - side));
                let scale = if n == last { pass.scale } else { pass.scale * e.scale };
                let info = [fx.info[0], k as f32, n as f32, 0.0];
                let fx = Fx { info, ..fx };
                previous =
                    self.pass(Shader::Fx(key, &pass.entry), rect, out, scale.clamp(0.05, 1.0), tex, fx);
                if let Some((id, _)) = front.filter(|_| out.is_some()) {
                    self.flip(id);
                }
            }
            outputs.push(previous);
        }
        if d.mask_mode == MaskMode::Mix && e.mask.is_some() {
            let base = self.r.pipes().chain(&self.r.device, std::iter::empty::<&Descriptor>(), false);
            let mut tex = EMPTY;
            (tex[0], tex[1], tex[2]) = (previous, mask, current);
            previous = self.pass(Shader::Fx(base, "fs_mask_mix"), rect, None, 1.0, tex, fx);
        }
        previous
    }
}
